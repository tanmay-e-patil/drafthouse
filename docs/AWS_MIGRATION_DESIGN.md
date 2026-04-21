# Drafthouse AWS Migration Design

Target scale: 900K DAU, 4.5B document versions. All AWS services. Region: `us-east-1` primary, `us-west-2` DR.

---

## Table of Contents

1. [Network Architecture](#1-network-architecture)
2. [Edge Layer](#2-edge-layer)
3. [Application Load Balancer](#3-application-load-balancer)
4. [ECS Fargate — Compute](#4-ecs-fargate--compute)
5. [RDS PostgreSQL](#5-rds-postgresql)
6. [RDS Proxy](#6-rds-proxy)
7. [ElastiCache Redis](#7-elasticache-redis)
8. [ScyllaDB on EC2](#8-scylladb-on-ec2)
9. [S3 — Object Storage](#9-s3--object-storage)
10. [SNS + SQS — Messaging](#10-sns--sqs--messaging)
11. [Export Worker](#11-export-worker)
12. [ECR — Container Registry](#12-ecr--container-registry)
13. [Secrets Manager](#13-secrets-manager)
14. [CI/CD — CodePipeline + CodeBuild](#14-cicd--codepipeline--codebuild)
15. [Observability](#15-observability)
16. [IAM Roles](#16-iam-roles)
17. [Security Groups](#17-security-groups)
18. [Backup & DR](#18-backup--dr)
19. [Cost Estimate](#19-cost-estimate)
20. [Migration Sequence](#20-migration-sequence)

---

## 1. Network Architecture

### VPC

| Parameter | Value |
|---|---|
| CIDR | `10.0.0.0/16` |
| Region | `us-east-1` |
| Availability Zones | `us-east-1a`, `us-east-1b`, `us-east-1c` |
| DNS hostnames | enabled |
| DNS resolution | enabled |
| VPC Flow Logs | enabled → CloudWatch Logs `/aws/vpc/flowlogs`, 7-day retention |

### Subnets

| Name | AZ | CIDR | Type | Hosts |
|---|---|---|---|---|
| `public-1a` | us-east-1a | `10.0.0.0/20` | Public | ALB, NAT GW |
| `public-1b` | us-east-1b | `10.0.16.0/20` | Public | ALB, NAT GW |
| `public-1c` | us-east-1c | `10.0.32.0/20` | Public | ALB, NAT GW |
| `app-1a` | us-east-1a | `10.0.48.0/20` | Private | ECS tasks |
| `app-1b` | us-east-1b | `10.0.64.0/20` | Private | ECS tasks |
| `app-1c` | us-east-1c | `10.0.80.0/20` | Private | ECS tasks |
| `data-1a` | us-east-1a | `10.0.96.0/20` | Private | RDS, Redis, ScyllaDB |
| `data-1b` | us-east-1b | `10.0.112.0/20` | Private | RDS, Redis, ScyllaDB |
| `data-1c` | us-east-1c | `10.0.128.0/20` | Private | RDS, Redis, ScyllaDB |

### NAT Gateways

One per AZ (3 total). Single NAT GW is cheaper but loses multi-AZ resilience — at 900K DAU, AZ failure without NAT GW failover takes down all outbound traffic from that AZ's ECS tasks.

| Parameter | Value |
|---|---|
| Count | 3 (one per public subnet) |
| Connectivity type | Public |
| Elastic IP | 1 per NAT GW |

### Route Tables

| Table | Associated subnets | Routes |
|---|---|---|
| `rt-public` | public-1a/1b/1c | `0.0.0.0/0` → Internet Gateway |
| `rt-app-1a` | app-1a | `0.0.0.0/0` → NAT GW in public-1a |
| `rt-app-1b` | app-1b | `0.0.0.0/0` → NAT GW in public-1b |
| `rt-app-1c` | app-1c | `0.0.0.0/0` → NAT GW in public-1c |
| `rt-data` | data-1a/1b/1c | `0.0.0.0/0` → NAT GW (any one, data tier has no direct egress except S3) |

### VPC Endpoints

| Endpoint | Type | Purpose |
|---|---|---|
| S3 Gateway | Gateway | Free, routes S3 traffic within AWS (no NAT GW cost) |
| Secrets Manager Interface | Interface | ECS tasks fetch secrets without NAT GW |
| ECR API Interface | Interface | ECS image pulls without NAT GW |
| ECR DKR Interface | Interface | Docker layer pulls |
| CloudWatch Logs Interface | Interface | Log shipping without NAT GW |
| SQS Interface | Interface | Collab + export worker SQS access |
| SNS Interface | Interface | Auth service SNS publish |
| XRay Interface | Interface | X-Ray trace shipping |

Interface endpoints cost ~$0.01/hr/AZ. Add to all 3 AZs for HA. Monthly: ~$65/endpoint × 8 endpoints = $520/month. Worth it — eliminates NAT GW data processing cost for high-volume traffic.

---

## 2. Edge Layer

### Route 53

| Record | Type | Value |
|---|---|---|
| `drafthouse.app` | A (Alias) | CloudFront distribution |
| `www.drafthouse.app` | A (Alias) | CloudFront distribution |
| `drafthouse.app` | MX | Resend MX records (email sending) |
| Health check | HTTPS | ALB `/health` endpoint, 30s interval, 3 failure threshold |

### ACM Certificates

| Certificate | Domains | Region |
|---|---|---|
| Primary | `drafthouse.app`, `*.drafthouse.app` | `us-east-1` (CloudFront requires us-east-1) |
| ALB | `drafthouse.app`, `*.drafthouse.app` | `us-east-1` (same region as ALB) |

Validation: DNS validation via Route 53 (auto-renews).

### AWS WAF v2

Attached to CloudFront distribution (not ALB — WAF at edge is cheaper, blocks sooner).

**Web ACL: `drafthouse-prod-wacl`**

| Rule name | Priority | Type | Action | Config |
|---|---|---|---|---|
| `AWSManagedRulesCommonRuleSet` | 10 | Managed rule group | Block | Override: `SizeRestrictions_BODY` → Count (large doc saves are legitimate) |
| `AWSManagedRulesKnownBadInputsRuleSet` | 20 | Managed rule group | Block | Default |
| `AWSManagedRulesSQLiRuleSet` | 30 | Managed rule group | Block | Default |
| `RateLimitLogin` | 40 | Rate-based | Block | URI path `/auth/login`, 50 req / 5min / IP |
| `RateLimitForgotPassword` | 50 | Rate-based | Block | URI path `/auth/forgot-password`, 10 req / 5min / IP |
| `RateLimitRegister` | 60 | Rate-based | Block | URI path `/auth/register`, 20 req / 5min / IP |
| `RateLimitGlobal` | 70 | Rate-based | Block | All URIs, 2000 req / 5min / IP |
| `AWSManagedRulesAmazonIpReputationList` | 80 | Managed rule group | Block | Default — blocks known malicious IPs |

**WAF settings:**
- CloudWatch metrics: enabled, metric name prefix `drafthouse`
- Sampled requests: enabled
- Log destination: CloudWatch Logs `/aws/waf/drafthouse`, 30-day retention

### CloudFront Distribution

**Origins:**

| Origin ID | Domain | Protocol | Path |
|---|---|---|---|
| `alb-origin` | ALB DNS name | HTTPS only | — |
| `s3-static-origin` | `drafthouse-static-prod.s3.us-east-1.amazonaws.com` | HTTPS only | — |

ALB origin: custom header `X-CloudFront-Secret: <random-32-char>` added by CloudFront. ALB listener rule rejects requests missing this header. Prevents ALB bypass.

**Cache Behaviors (evaluated in priority order):**

| Path pattern | Origin | Cache policy | TTL | Compress | WS |
|---|---|---|---|---|---|
| `/_next/static/*` | s3-static | `CachingOptimized` | max (31536000s) | yes | no |
| `/fonts/*` | s3-static | `CachingOptimized` | max (31536000s) | yes | no |
| `/collab/*` | alb-origin | `CachingDisabled` | 0 | no | **yes** |
| `/auth/*` | alb-origin | `CachingDisabled` | 0 | yes | no |
| `/documents/*` | alb-origin | `CachingDisabled` | 0 | yes | no |
| `/invites/*` | alb-origin | `CachingDisabled` | 0 | yes | no |
| `Default (/*)`  | alb-origin | Custom (30s for public, 0 for auth'd) | 30s | yes | no |

**Distribution settings:**

| Parameter | Value |
|---|---|
| Price class | `PriceClass_100` (US, Canada, Europe) — upgrade to `PriceClass_All` post launch |
| HTTP version | HTTP/2 and HTTP/3 |
| IPv6 | enabled |
| Minimum TLS version | `TLSv1.2_2021` |
| Security policy | `TLSv1.2_2021` |
| Compress objects | yes |
| WAF Web ACL | `drafthouse-prod-wacl` |
| Access logging | S3 bucket `drafthouse-cf-logs-prod`, prefix `cf/` |
| Certificate | ACM `drafthouse.app` (us-east-1) |
| Alternate domain names | `drafthouse.app`, `www.drafthouse.app` |

**WebSocket on `/collab/*`:**
CloudFront supports WebSocket natively when using HTTP/1.1 upgrade headers. No extra config beyond disabling caching on that path pattern.

---

## 3. Application Load Balancer

**ALB: `drafthouse-prod-alb`**

| Parameter | Value |
|---|---|
| Scheme | `internet-facing` (CloudFront is in front; ALB must be reachable by CF edge nodes) |
| IP address type | IPv4 |
| Subnets | public-1a, public-1b, public-1c |
| Security group | `alb-sg` |
| Deletion protection | enabled |
| Access logs | S3 `drafthouse-alb-logs-prod`, prefix `alb/`, 7-day S3 lifecycle |
| Idle timeout | 4000s (must exceed WS keepalive; default 60s kills WS connections) |

**Listeners:**

| Port | Protocol | Default action |
|---|---|---|
| 80 | HTTP | Redirect to HTTPS 443, 301 |
| 443 | HTTPS | Forward rules (see below) |

HTTPS listener certificate: ACM `drafthouse.app`.

**Listener Rules (HTTPS:443, priority order):**

| Priority | Condition | Action |
|---|---|---|
| 1 | Missing header `X-CloudFront-Secret` | Fixed response 403 |
| 10 | Path `/auth/*` | Forward → `tg-auth` |
| 20 | Path `/documents/*` OR `/invites/*` | Forward → `tg-documents` |
| 30 | Path `/collab/*` | Forward → `tg-collab` |
| 40 | Path `/health` | Fixed response 200 (health check for Route 53) |
| Default | — | Forward → `tg-documents` |

**Target Groups:**

`tg-auth`:
| Parameter | Value |
|---|---|
| Protocol | HTTP |
| Port | 8080 |
| Target type | ip (Fargate) |
| Health check path | `/auth/health` |
| Health check interval | 30s |
| Health check timeout | 5s |
| Healthy threshold | 2 |
| Unhealthy threshold | 3 |
| Deregistration delay | 30s |
| Load balancing algorithm | Least outstanding requests |

`tg-documents`:
| Parameter | Value |
|---|---|
| Protocol | HTTP |
| Port | 8080 |
| Target type | ip |
| Health check path | `/documents/health` |
| Health check interval | 30s |
| Deregistration delay | 30s |
| Load balancing algorithm | Least outstanding requests |

`tg-collab`:
| Parameter | Value |
|---|---|
| Protocol | HTTP |
| Port | 8080 |
| Target type | ip |
| Health check path | `/collab/health` |
| Health check interval | 30s |
| Deregistration delay | 300s (allow WS drain) |
| Load balancing algorithm | Least outstanding requests |
| Stickiness | disabled — see note below |

**Collab routing note:** ALB stickiness by cookie routes a user to the same task, not a document to the same task. All editors for doc X must land on one process. Solution: collab tasks use Redis pub/sub relay. Every collab task subscribes to `collab:room:{doc_id}` Redis channel. WS message from any client → published to Redis → all subscribing tasks relay to their local WS connections for that doc. State (DocRoom DashMap) stays per-process; ops are relayed cross-process via Redis. Eliminates sticky routing requirement entirely. Write latency: +0.5-1ms for Redis relay roundtrip. Acceptable.

---

## 4. ECS Fargate — Compute

All services: platform version `LATEST` (1.4.0+), architecture `arm64` (Graviton3, 20% cheaper than x86 on Fargate, better perf/dollar for Rust workloads).

### Auth Service

**Task Definition: `drafthouse-auth`**

| Parameter | Value |
|---|---|
| CPU | 2048 (2 vCPU) |
| Memory | 4096 MB |
| Architecture | arm64 |
| Network mode | awsvpc |
| Task execution role | `ecs-task-execution-role` |
| Task role | `ecs-auth-task-role` |

Why 4GB: argon2id `memory=64MB` per hash. 50 concurrent logins = 3.2GB peak. 4GB gives headroom.

Container definition (`auth`):
| Parameter | Value |
|---|---|
| Image | `ECR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/drafthouse/auth:latest` |
| Port | 8080 |
| Essential | true |
| CPU | 1792 (leave 256 for sidecar) |
| Memory | 3800 MB (leave 296 for sidecar) |
| Log driver | `awslogs` |
| Log group | `/ecs/auth` |
| Log stream prefix | `auth` |
| Environment | via Secrets Manager (see §13) |
| Health check | `CMD-SHELL curl -f http://localhost:8080/auth/health || exit 1`, interval 30s, timeout 5s, retries 3, startPeriod 60s |

Container definition (`xray-daemon` sidecar):
| Parameter | Value |
|---|---|
| Image | `public.ecr.aws/xray/aws-xray-daemon:latest` |
| CPU | 256 |
| Memory | 256 MB |
| Port | 2000/udp |
| Essential | false |

**ECS Service: `auth-service`**

| Parameter | Value |
|---|---|
| Launch type | Fargate |
| Desired count | 3 |
| Subnets | app-1a, app-1b, app-1c |
| Security group | `ecs-auth-sg` |
| Assign public IP | disabled |
| Load balancer | `tg-auth` |
| Deployment type | Rolling update |
| Min healthy percent | 50 |
| Max percent | 200 |
| Circuit breaker | enabled, rollback enabled |
| Health check grace period | 60s |

Auto-scaling:
| Parameter | Value |
|---|---|
| Min capacity | 3 |
| Max capacity | 20 |
| Policy type | Target tracking |
| Metric | `ALBRequestCountPerTarget` |
| Target value | 800 requests/target (auth is expensive per-request due to argon2id) |
| Scale-in cooldown | 300s |
| Scale-out cooldown | 60s |

---

### Documents Service

**Task Definition: `drafthouse-documents`**

| Parameter | Value |
|---|---|
| CPU | 512 (0.5 vCPU) |
| Memory | 1024 MB |
| Architecture | arm64 |

Documents service is DB-bound, not CPU/memory bound. Most time spent waiting on RDS Proxy → RDS.

Container definition (`documents`):
| Parameter | Value |
|---|---|
| Image | `ECR/drafthouse/documents:latest` |
| Port | 8080 |
| CPU | 256 |
| Memory | 768 MB |
| Log group | `/ecs/documents` |

**ECS Service: `documents-service`**

| Parameter | Value |
|---|---|
| Desired count | 3 |
| Min healthy percent | 50 |
| Max percent | 200 |
| Circuit breaker | enabled, rollback enabled |

Auto-scaling:
| Parameter | Value |
|---|---|
| Min capacity | 3 |
| Max capacity | 30 |
| Policy type | Target tracking |
| Metric | `ALBRequestCountPerTarget` |
| Target value | 1500 |
| Scale-in cooldown | 300s |
| Scale-out cooldown | 60s |

---

### Collab Service

**Task Definition: `drafthouse-collab`**

| Parameter | Value |
|---|---|
| CPU | 4096 (4 vCPU) |
| Memory | 16384 MB (16 GB) |
| Architecture | arm64 |

Memory sizing: 135K concurrent WS / 10 tasks = 13.5K WS/task. Each WS connection ~80KB overhead (Actix WS buffers, Yrs awareness state) = 1.08GB. Active Yrs docs: ~2000 docs/task × 200KB avg doc = 400MB. Redis pub/sub relay buffers + Tokio runtime overhead: ~1GB. Total: ~2.5GB working set. 16GB gives large safety margin; Yrs doc memory is proportional to doc complexity, not predictable.

Container definition (`collab`):
| Parameter | Value |
|---|---|
| Image | `ECR/drafthouse/collab:latest` |
| Port | 8080 |
| CPU | 3840 |
| Memory | 15872 MB |
| Log group | `/ecs/collab` |
| Ulimits | `nofile` soft 65536 hard 65536 (WS connections need file descriptors) |

Container definition (`xray-daemon` sidecar): same as auth.

Container definition (`adot-collector` sidecar — Prometheus metrics):
| Parameter | Value |
|---|---|
| Image | `public.ecr.aws/aws-observability/aws-otel-collector:latest` |
| CPU | 256 |
| Memory | 512 MB |
| Config | scrape `/metrics` on localhost:9090, remote write to AMP workspace |

**ECS Service: `collab-service`**

| Parameter | Value |
|---|---|
| Desired count | 10 |
| Min healthy percent | 80 (don't kill >20% of tasks during deploy — minimize WS reconnect storm) |
| Max percent | 120 |
| Circuit breaker | enabled, rollback enabled |
| Health check grace period | 120s (Yrs state takes time to initialize on cold start) |
| Deregistration delay | 300s (allow WS connections to drain gracefully) |

Auto-scaling (step scaling, not target tracking — WS connections are a better signal than request count):
| Parameter | Value |
|---|---|
| Min capacity | 10 |
| Max capacity | 50 |
| Metric | Custom CloudWatch `drafthouse/active_ws_connections` (emitted by collab tasks) |
| Step 1 | Add 3 tasks when avg connections/task > 11000 |
| Step 2 | Add 5 tasks when avg connections/task > 13000 |
| Step 3 (aggressive) | Add 8 tasks when avg connections/task > 14000 |
| Scale-in | Remove 2 tasks when avg connections/task < 6000 for 10 min |
| Scale-in cooldown | 600s (don't scale in too fast during partial reconnects) |

---

### Export Worker

**Task Definition: `drafthouse-export-worker`**

| Parameter | Value |
|---|---|
| CPU | 1024 (1 vCPU) |
| Memory | 4096 MB (ZIP of large doc collections can be multi-GB in memory) |
| Architecture | arm64 |

**ECS Service: `export-worker-service`**

| Parameter | Value |
|---|---|
| Desired count | 2 |
| Min healthy percent | 0 (can scale to 0 when queue empty) |
| Max percent | 200 |

Auto-scaling:
| Parameter | Value |
|---|---|
| Min capacity | 0 |
| Max capacity | 20 |
| Policy type | Step scaling on SQS `ApproximateNumberOfMessagesVisible` |
| Step 1 | Add 2 tasks when queue depth > 5 |
| Step 2 | Add 5 tasks when queue depth > 50 |
| Scale-in | Remove 1 task when queue depth = 0 for 5 min |

---

## 5. RDS PostgreSQL

**DB Instance: `drafthouse-prod-pg`**

| Parameter | Value |
|---|---|
| Engine | PostgreSQL 16.3 |
| Instance class | `db.r7g.2xlarge` (8 vCPU, 64 GB RAM) |
| Multi-AZ | Yes (synchronous standby in different AZ) |
| Storage type | gp3 |
| Allocated storage | 500 GB |
| Max allocated storage | 2000 GB (auto-scaling enabled) |
| IOPS (gp3) | 12000 |
| Storage throughput | 500 MB/s |
| Encryption | enabled, KMS key `drafthouse/rds` |
| Backup retention | 7 days |
| Backup window | 02:00-03:00 UTC |
| Maintenance window | Mon 03:00-04:00 UTC |
| Auto minor version upgrade | disabled (control upgrade timing) |
| Deletion protection | enabled |
| Enhanced monitoring | enabled, 15s interval |
| Performance Insights | enabled, 7-day retention |
| CloudWatch log exports | postgresql (slow query log), upgrade |
| Publicly accessible | No |
| Subnet group | `drafthouse-rds-subnet-group` (data-1a, data-1b, data-1c) |
| Security group | `rds-sg` |
| CA certificate | `rds-ca-rsa4096-g1` |

**Parameter Group: `drafthouse-pg16`**

| Parameter | Value | Reason |
|---|---|---|
| `shared_buffers` | `16384MB` (16 GB = 25% of RAM) | Postgres buffer pool |
| `effective_cache_size` | `49152MB` (48 GB) | Query planner hint |
| `max_connections` | `500` | RDS Proxy sits in front; tasks connect to Proxy not directly |
| `work_mem` | `65536` (64 MB) | Sort/hash ops per query |
| `maintenance_work_mem` | `2097152` (2 GB) | VACUUM, CREATE INDEX |
| `wal_level` | `replica` | Enables read replicas |
| `max_wal_senders` | `10` | Read replica streams |
| `checkpoint_completion_target` | `0.9` | Spread checkpoint I/O |
| `random_page_cost` | `1.1` | Tuned for SSD (default 4.0 is for HDD) |
| `effective_io_concurrency` | `200` | SSD parallel I/O |
| `max_parallel_workers_per_gather` | `4` | Parallel seq scans |
| `max_parallel_workers` | `8` | Total parallel workers |
| `password_encryption` | `scram-sha-256` | Auth protocol |
| `log_min_duration_statement` | `200` | Log queries > 200ms |
| `log_checkpoints` | `on` | Monitor checkpoint frequency |
| `log_connections` | `off` | Too noisy at scale |
| `log_lock_waits` | `on` | Detect contention |
| `deadlock_timeout` | `500` | 500ms deadlock detection |
| `idle_in_transaction_session_timeout` | `30000` | Kill idle-in-transaction after 30s |
| `statement_timeout` | `30000` | Kill queries > 30s |

**Read Replicas:**

`drafthouse-prod-pg-replica-1` (`db.r7g.xlarge`, 4 vCPU, 32 GB):
- Purpose: document list queries, member queries from documents service
- Multi-AZ: No (replica — standby promotion handled by primary Multi-AZ)

`drafthouse-prod-pg-replica-2` (`db.r7g.large`, 2 vCPU, 16 GB):
- Purpose: GDPR export worker reads, analytics
- Multi-AZ: No

**Critical indexes (add via migration, not in schema definition):**

```sql
-- Document list query: WHERE owner_id = $1 AND id < $cursor ORDER BY id DESC LIMIT 20
CREATE INDEX CONCURRENTLY idx_documents_owner_cursor
  ON documents (owner_id, id DESC);

-- Member lookup: all docs a user belongs to
CREATE INDEX CONCURRENTLY idx_document_members_user
  ON document_members (user_id);

-- Refresh token lookup on login/refresh
CREATE INDEX CONCURRENTLY idx_refresh_tokens_user_expiry
  ON refresh_tokens (user_id, expires_at)
  WHERE expires_at > now();

-- Invite link: cleanup job for expired links
CREATE INDEX CONCURRENTLY idx_invite_links_expiry
  ON invite_links (expires_at)
  WHERE expires_at IS NOT NULL;
```

---

## 6. RDS Proxy

**Proxy: `drafthouse-prod-rds-proxy`**

| Parameter | Value |
|---|---|
| Engine | PostgreSQL |
| DB instance | `drafthouse-prod-pg` (primary) |
| IAM authentication | enabled |
| Secrets Manager secret | `drafthouse/rds/credentials` |
| Require TLS | Yes |
| Idle client timeout | 1800s |
| VPC | drafthouse VPC |
| Subnets | data-1a, data-1b, data-1c |
| Security group | `rds-proxy-sg` |

**Connection pool config:**

| Parameter | Value | Reason |
|---|---|---|
| `MaxConnectionsPercent` | 80 | 80% of `max_connections` (500) = 400 connections from proxy to RDS |
| `MaxIdleConnectionsPercent` | 50 | Keep 50% of max connections warm when idle |
| `ConnectionBorrowTimeout` | 120s | Max time ECS task waits for connection from pool |

**Read replica proxy: `drafthouse-prod-rds-proxy-ro`**

Same config, targets `drafthouse-prod-pg-replica-1`. Documents service and export worker connect to this endpoint for read queries. Application-level: use separate `DATABASE_URL_RO` env var.

---

## 7. ElastiCache Redis

**Cluster: `drafthouse-prod-redis`**

Redis is now mandatory for: collab WS pub/sub relay, WS ticket storage (replacing `ws_tickets` Postgres table), editor count coordination, rate limiter state, snapshot metadata cache.

| Parameter | Value |
|---|---|
| Engine | Redis OSS 7.2 |
| Mode | Cluster mode enabled |
| Shards | 3 |
| Replicas per shard | 1 (1 primary + 1 replica per shard = 6 nodes total) |
| Node type | `cache.r7g.xlarge` (4 vCPU, 26.32 GB RAM) |
| Multi-AZ | enabled (primary and replica in different AZs) |
| Auto-failover | enabled |
| At-rest encryption | enabled (KMS key `drafthouse/redis`) |
| In-transit encryption | enabled (TLS) |
| AUTH token | enabled (stored in Secrets Manager `drafthouse/redis/auth-token`) |
| Subnet group | `drafthouse-redis-subnet-group` (data-1a, data-1b, data-1c) |
| Security group | `redis-sg` |
| Backup | enabled, daily snapshot, 7-day retention |
| Backup window | 04:00-05:00 UTC |
| Maintenance window | Tue 03:00-04:00 UTC |
| Auto minor version upgrade | disabled |
| Log delivery | slow log → CloudWatch `/aws/elasticache/redis/slow`, engine log → CloudWatch |

**Parameter Group: `drafthouse-redis7`**

| Parameter | Value | Reason |
|---|---|---|
| `maxmemory-policy` | `allkeys-lru` | Evict LRU keys when full — WS tickets/rate limiters are ephemeral |
| `maxmemory` | `22gb` | Leave headroom on 26GB nodes for OS + Redis overhead |
| `timeout` | `300` | Close idle connections after 5min |
| `tcp-keepalive` | `60` | Detect dead connections every 60s |
| `lazyfree-lazy-eviction` | `yes` | Non-blocking eviction |
| `lazyfree-lazy-expire` | `yes` | Non-blocking expiry |
| `lazyfree-lazy-server-del` | `yes` | Non-blocking DEL |
| `activerehashing` | `yes` | Incremental rehashing |
| `notify-keyspace-events` | `Ex` | Keyspace expiry events (for WS ticket expiry monitoring if needed) |
| `cluster-node-timeout` | `15000` | 15s before node considered failed |
| `hz` | `15` | Background task frequency (default 10, increase for faster expiry) |
| `aof-use-rdb-preamble` | `yes` | Faster AOF rewrite |
| `list-max-ziplist-size` | `-2` | 8KB max ziplist (pub/sub message lists) |

**Redis key schema:**

| Key pattern | Type | TTL | Purpose |
|---|---|---|---|
| `ws_ticket:{token_hash}` | String (`{doc_id}:{user_id}`) | 30s | WS ticket (replaces Postgres table) |
| `collab:room:{doc_id}:editors` | String (count) | none | Active editor count per doc |
| `collab:room:{doc_id}` | Pub/Sub channel | — | WS op relay between collab tasks |
| `snapshot_meta:{doc_id}` | Hash (version, s3_key, taken_at) | 300s | Snapshot metadata cache (avoid ScyllaDB cold reads) |
| `ratelimit:{ip}:{endpoint}` | String (count) | sliding window | App-level rate limiting (WAF handles edge, this handles internal) |

---

## 8. ScyllaDB on EC2

Not using ScyllaDB Cloud: need full control over `scylla.yaml`, compaction strategy, and JMX metrics export for CloudWatch. At this scale, custom tuning outweighs managed convenience.

**EC2 Instance: `drafthouse-scylla-{1,2,3}`**

| Parameter | Value |
|---|---|
| Instance type | `i4i.4xlarge` (16 vCPU, 128 GB RAM, 2× 1.875 TB NVMe SSD) |
| Count | 3 (one per AZ: data-1a, data-1b, data-1c) |
| AMI | ScyllaDB 5.4 official AMI (`ami-*` — check ScyllaDB downloads page for latest us-east-1 AMI) |
| Storage | Instance store NVMe (not EBS — ScyllaDB I/O scheduler optimized for NVMe, EBS adds 0.5-2ms latency per op) |
| Placement group | `spread` (one node per distinct hardware rack, maximizes fault isolation) |
| Subnet | one node per data-1a, data-1b, data-1c |
| Security group | `scylla-sg` |
| IAM instance profile | `scylla-instance-profile` (S3 write for backups, SSM for Run Command) |
| Termination protection | enabled |
| EBS root volume | 100 GB gp3 (OS only, not data) |

**`/etc/scylla/scylla.yaml` (key parameters):**

```yaml
cluster_name: "drafthouse-prod"
num_tokens: 256
seeds: "10.0.96.X,10.0.112.X,10.0.128.X"   # private IPs of all 3 nodes

# Partitioner
partitioner: org.apache.cassandra.dht.Murmur3Partitioner

# Compaction
# Ops table: SizeTieredCompactionStrategy (append-heavy, good for time-series WAL)
# Snapshots table: LeveledCompactionStrategy (small, updated in-place, good read perf)

# Storage
data_file_directories:
  - /var/lib/scylla/data
commitlog_directory: /var/lib/scylla/commitlog
commitlog_sync: batch
commitlog_sync_batch_window_in_ms: 1     # match app-level 100ms WAL buffer
commitlog_total_space_in_mb: 10240       # 10 GB commitlog

# Memory
developer_mode: false
# ScyllaDB auto-detects RAM; uses ~50% for cache by default on 128GB = 64GB cache

# Network
rpc_address: 0.0.0.0
broadcast_rpc_address: "<private_ip>"
listen_address: "<private_ip>"
native_transport_port: 9042

# Auth
authenticator: PasswordAuthenticator
authorizer: CassandraAuthorizer

# TLS (inter-node)
server_encryption_options:
  internode_encryption: all
  certificate: /etc/scylla/ssl/scylla.crt
  keyfile: /etc/scylla/ssl/scylla.key
  truststore: /etc/scylla/ssl/ca.crt

# TLS (client)
client_encryption_options:
  enabled: true
  certificate: /etc/scylla/ssl/scylla.crt
  keyfile: /etc/scylla/ssl/scylla.key

# Memtable
memtable_allocation_type: offheap_objects   # reduces GC pressure (ScyllaDB is C++, this is for Seastar allocator)

# Compaction throttle
compaction_throughput_mb_per_sec: 400       # limit compaction I/O to 400 MB/s, leave headroom for reads/writes

# Snapshot retention (not ScyllaDB-native — handled by backup script)

# Hinted handoff (repair for missed writes during node downtime)
hinted_handoff_enabled: true
max_hint_window_in_ms: 10800000            # 3 hours

# Row cache (for snapshots table — frequently read)
row_cache_size_in_mb: 4096                 # 4 GB row cache for snapshots
```

**Table-level compaction strategies:**

```cql
-- WAL ops: append-only, never updated, TTL drives expiry
ALTER TABLE drafthouse.ops
  WITH compaction = {
    'class': 'SizeTieredCompactionStrategy',
    'min_threshold': 4,
    'max_threshold': 32
  }
  AND default_time_to_live = 172800;   -- 48hr TTL (reduced from 7 days — see §2 analysis)

-- Snapshots: small table, ring buffer of 5 per doc, always read in full
ALTER TABLE drafthouse.snapshots
  WITH compaction = {
    'class': 'LeveledCompactionStrategy',
    'sstable_size_in_mb': 160
  }
  AND caching = {
    'keys': 'ALL',
    'rows_per_partition': 'ALL'   -- cache all 5 versions per doc partition
  };
```

**Backup script (via SSM Run Command, hourly cron on each node):**

```bash
#!/bin/bash
set -euo pipefail
SNAPSHOT_TAG="hourly-$(date +%Y%m%d-%H%M%S)"
BUCKET="drafthouse-backups-prod"

nodetool snapshot -t "$SNAPSHOT_TAG" drafthouse

# Upload only the new snapshot SSTables
find /var/lib/scylla/data/drafthouse -name "snapshots/$SNAPSHOT_TAG" -type d | \
  while read -r dir; do
    table=$(echo "$dir" | awk -F'/' '{print $(NF-2)}')
    aws s3 sync "$dir" "s3://$BUCKET/scylla/$(hostname)/$table/$SNAPSHOT_TAG/" \
      --storage-class STANDARD_IA
  done

# Delete snapshot from local disk (free space)
nodetool clearsnapshot -t "$SNAPSHOT_TAG" drafthouse
```

SSM document: `drafthouse-scylla-backup`, run via EventBridge schedule every hour on all 3 nodes.

**Monitoring (CloudWatch agent on each ScyllaDB node):**

ScyllaDB exposes Prometheus metrics on port 9180. CloudWatch agent scrapes and ships:
- `scylla_storage_proxy_coordinator_write_latency` → `ScyllaDB/WriteLatency`
- `scylla_storage_proxy_coordinator_read_latency` → `ScyllaDB/ReadLatency`
- `scylla_compaction_pending_compactions` → `ScyllaDB/PendingCompactions`
- `scylla_lsa_large_allocation` → `ScyllaDB/LargeAllocations`

---

## 9. S3 — Object Storage

### `drafthouse-snapshots-prod`

| Parameter | Value |
|---|---|
| Purpose | Yrs doc snapshot blobs (moved from ScyllaDB) |
| Versioning | disabled (objects are immutable, keyed by doc_id+version+epoch) |
| Encryption | SSE-S3 (AES-256) |
| Block public access | all blocked |
| VPC endpoint | S3 Gateway endpoint (free) |
| Transfer acceleration | disabled (all access internal) |
| Object key format | `snapshots/{doc_id}/{version}/{taken_at_unix_ms}` |
| Request Requester Pays | disabled |

Lifecycle rules:
| Rule | Transition | Days |
|---|---|---|
| To Standard-IA | All objects | 30 |
| To Glacier Instant Retrieval | All objects | 90 |
| Expire | All objects | 365 |

### `drafthouse-exports-prod`

| Parameter | Value |
|---|---|
| Purpose | GDPR ZIP exports (temporary, pre-signed URL delivery) |
| Versioning | disabled |
| Encryption | SSE-KMS (`drafthouse/exports`) |
| Block public access | all blocked |
| Object key format | `exports/{user_id}/{request_id}.zip` |

Lifecycle rules:
| Rule | Action | Days |
|---|---|---|
| Expire | Delete export ZIPs | 7 |

Pre-signed URL expiry: 604800s (7 days), generated by export worker after upload.

### `drafthouse-static-prod`

| Parameter | Value |
|---|---|
| Purpose | Frontend static assets (JS, CSS, fonts, icons) |
| Versioning | enabled |
| Encryption | SSE-S3 |
| Block public access | all blocked (CloudFront OAC only) |
| CloudFront OAC | `drafthouse-static-oac`, signing protocol SigV4 |

Objects deployed by CodePipeline build step: `pnpm build → aws s3 sync dist/ s3://drafthouse-static-prod/`.

Cache-Control headers set at upload:
- `/_next/static/**`: `max-age=31536000, immutable`
- `/fonts/**`: `max-age=31536000, immutable`
- `/*.html`: `max-age=0, no-cache, no-store`

### `drafthouse-backups-prod`

| Parameter | Value |
|---|---|
| Purpose | pg_dump daily, ScyllaDB nodetool snapshots hourly |
| Versioning | enabled |
| Encryption | SSE-KMS (`drafthouse/backups`) |
| Block public access | all blocked |
| Object Lock | governance mode, retain for 30 days |
| Replication | Cross-region replication → `drafthouse-backups-dr` in `us-west-2` |

Lifecycle rules:
| Rule | Transition | Days |
|---|---|---|
| To Standard-IA | All objects | 7 |
| To Glacier Flexible Retrieval | All objects | 30 |
| Expire | All objects | 365 |

### `drafthouse-alb-logs-prod` + `drafthouse-cf-logs-prod`

S3 buckets for ALB and CloudFront access logs. No versioning. Lifecycle: expire after 30 days. Encryption: SSE-S3.

---

## 10. SNS + SQS — Messaging

Replaces in-process `publish_event!` macros. Two event flows.

### TitleUpdated Flow

**SNS Topic: `drafthouse-title-updated`**

| Parameter | Value |
|---|---|
| Type | Standard |
| Encryption | SSE, AWS managed key (`alias/aws/sns`) |
| Access policy | Allow `ecs-documents-task-role` to `sns:Publish` |

**SQS Queue: `drafthouse-collab-title-updates`**

| Parameter | Value |
|---|---|
| Type | Standard |
| Visibility timeout | 30s |
| Message retention | 3600s (1 hour — stale title update after 1hr is irrelevant) |
| Receive message wait time | 20s (long polling) |
| Encryption | SSE, AWS managed key |
| Redrive policy | DLQ after 3 receive attempts |

**DLQ: `drafthouse-collab-title-updates-dlq`**

| Parameter | Value |
|---|---|
| Type | Standard |
| Message retention | 345600s (4 days) |

SNS subscription: `drafthouse-title-updated` → `drafthouse-collab-title-updates`, raw message delivery enabled.

Collab service polls this queue in a background Tokio task. On receipt: look up `doc_id` in local DashMap → if room exists, publish `title_update` WS message to Redis pub/sub channel `collab:room:{doc_id}` → all collab tasks relay to their connected clients.

### ExportRequested Flow

**SNS Topic: `drafthouse-export-requested`**

| Parameter | Value |
|---|---|
| Type | Standard |
| Encryption | SSE |

**SQS Queue: `drafthouse-export-jobs.fifo`**

| Parameter | Value |
|---|---|
| Type | FIFO |
| Content-based deduplication | enabled (dedup by message body = user_id — prevents double export) |
| FIFO throughput limit | `perMessageGroupId` (one export per user in flight) |
| Message group ID | `{user_id}` (set by auth service at publish time) |
| Visibility timeout | 600s (export worker has 10 min before message reappears) |
| Message retention | 86400s (24 hours) |
| Receive message wait time | 20s |
| Encryption | SSE-KMS (`drafthouse/exports`) — messages contain user PII metadata |
| Redrive policy | DLQ after 3 attempts |

**DLQ: `drafthouse-export-jobs-dlq.fifo`**

| Parameter | Value |
|---|---|
| Type | FIFO |
| Message retention | 1209600s (14 days) — GDPR compliance: failed exports must be retriable |

DLQ alarm: CloudWatch alarm on `ApproximateNumberOfMessagesVisible > 0` → SNS alert to ops team. Every failed export is a GDPR obligation.

---

## 11. Export Worker

Detailed in §4 (ECS service). Additional config:

**SQS consumption:**

Export worker task polls `drafthouse-export-jobs.fifo` via long polling (20s wait). On receive:
1. Parse `{ user_id, email, request_id }` from message body
2. `SELECT id FROM documents WHERE owner_id = $user_id` via RDS Proxy read replica
3. For each doc: fetch latest snapshot S3 key from Redis cache or ScyllaDB → `s3:GetObject` from `drafthouse-snapshots-prod`
4. Decode Yrs snapshot blob → extract markdown text (Yrs doc → Text type → string)
5. Build ZIP in memory: one `.md` file per document, named by title
6. `s3:PutObject` to `drafthouse-exports-prod/{user_id}/{request_id}.zip`
7. Generate pre-signed URL (7 day expiry)
8. Call Resend API: send email with pre-signed URL link
9. `sqs:DeleteMessage`

Visibility timeout extension: if processing takes > 500s (near 600s timeout), call `ChangeMessageVisibility` to extend by another 600s. Prevents re-delivery mid-export.

---

## 12. ECR — Container Registry

**Repositories:**

| Repository | Scan on push | Tag mutability | Purpose |
|---|---|---|---|
| `drafthouse/auth` | enabled (enhanced, Inspector v2) | immutable | Auth service |
| `drafthouse/documents` | enabled | immutable | Documents service |
| `drafthouse/collab` | enabled | immutable | Collab service |
| `drafthouse/export-worker` | enabled | immutable | Export worker |
| `drafthouse/migrate-pg` | enabled | immutable | Postgres migration runner |
| `drafthouse/migrate-scylla` | enabled | immutable | ScyllaDB migration runner |

**Lifecycle policy (applied to all repos):**

```json
{
  "rules": [
    {
      "rulePriority": 1,
      "description": "Keep last 10 tagged images",
      "selection": {
        "tagStatus": "tagged",
        "tagPatternList": ["*"],
        "countType": "imageCountMoreThan",
        "countNumber": 10
      },
      "action": { "type": "expire" }
    },
    {
      "rulePriority": 2,
      "description": "Delete untagged images older than 1 day",
      "selection": {
        "tagStatus": "untagged",
        "countType": "sinceImagePushed",
        "countUnit": "days",
        "countNumber": 1
      },
      "action": { "type": "expire" }
    }
  ]
}
```

**Cross-region replication to `us-west-2`:** configured on each repository for DR (redeploy from us-west-2 without re-pushing images).

---

## 13. Secrets Manager

**Secret: `drafthouse/auth/env`**

```json
{
  "DATABASE_URL": "postgresql://app_user:...@drafthouse-prod-rds-proxy.proxy-xxx.us-east-1.rds.amazonaws.com:5432/drafthouse?sslmode=require",
  "JWT_SECRET": "...",
  "JWT_EXPIRY_SECS": "900",
  "REFRESH_TOKEN_EXPIRY_DAYS": "30",
  "RESEND_API_KEY": "re_...",
  "APP_ORIGIN": "https://drafthouse.app",
  "SNS_EXPORT_TOPIC_ARN": "arn:aws:sns:us-east-1:ACCOUNT:drafthouse-export-requested"
}
```

**Secret: `drafthouse/documents/env`**

```json
{
  "DATABASE_URL": "postgresql://app_user:...@proxy...:5432/drafthouse?sslmode=require",
  "DATABASE_URL_RO": "postgresql://app_user:...@proxy-ro...:5432/drafthouse?sslmode=require",
  "JWT_SECRET": "...",
  "WS_TICKET_EXPIRY_SECS": "30",
  "SNS_TITLE_UPDATED_TOPIC_ARN": "arn:aws:sns:us-east-1:ACCOUNT:drafthouse-title-updated",
  "REDIS_URL": "rediss://:authtoken@drafthouse-prod-redis.xxx.cfg.use1.cache.amazonaws.com:6379"
}
```

**Secret: `drafthouse/collab/env`**

```json
{
  "SCYLLA_NODES": "10.0.96.X,10.0.112.X,10.0.128.X",
  "SCYLLA_USERNAME": "collab_service",
  "SCYLLA_PASSWORD": "...",
  "SCYLLA_KEYSPACE": "drafthouse",
  "REDIS_URL": "rediss://:authtoken@...cfg.use1.cache.amazonaws.com:6379",
  "SQS_TITLE_UPDATES_URL": "https://sqs.us-east-1.amazonaws.com/ACCOUNT/drafthouse-collab-title-updates",
  "SNAPSHOT_OPS_THRESHOLD": "100",
  "SNAPSHOT_INTERVAL_SECS": "30",
  "DOC_MAX_BYTES": "1048576",
  "DOC_MSG_MAX_BYTES": "102400",
  "S3_SNAPSHOTS_BUCKET": "drafthouse-snapshots-prod",
  "EDITOR_CAP": "100"
}
```

**Secret: `drafthouse/rds/credentials`** (used by RDS Proxy, auto-rotated):

```json
{
  "username": "app_user",
  "password": "...",
  "engine": "postgres",
  "host": "drafthouse-prod-pg.xxx.us-east-1.rds.amazonaws.com",
  "port": 5432,
  "dbname": "drafthouse"
}
```

Rotation: enabled, 30-day rotation, RDS-managed Lambda rotator.

**Secret: `drafthouse/redis/auth-token`** — Redis AUTH token, manual rotation (requires cluster restart, schedule with maintenance window).

ECS task secrets injection: in task definition, use `secrets` field pointing to specific Secrets Manager ARNs → injected as env vars at container start. No secrets in environment variables at rest.

---

## 14. CI/CD — CodePipeline + CodeBuild

### CodePipeline: `drafthouse-prod-pipeline`

**Stage 1 — Source:**
| Parameter | Value |
|---|---|
| Provider | GitHub v2 (via CodeStar connection) |
| Repository | `yourorg/drafthouse` |
| Branch | `main` |
| Change detection | webhook (instant, no polling) |
| Output artifact | `SourceArtifact` |

**Stage 2 — Build:**

CodeBuild project: `drafthouse-build`

| Parameter | Value |
|---|---|
| Environment image | `aws/codebuild/amazonlinux2-aarch64-standard:3.0` (arm64, matches Fargate arm64) |
| Compute type | `BUILD_GENERAL1_LARGE` (8 vCPU, 16 GB RAM — Rust full workspace build needs this) |
| Privileged mode | enabled (testcontainers runs Docker) |
| Timeout | 45 min |
| Cache type | S3, cache key `hash:files:Cargo.lock` |
| Cache paths | `/root/.cargo/registry`, `/root/.cargo/git`, `target/` |
| Service role | `codebuild-drafthouse-role` |
| VPC | drafthouse VPC (app subnets) — needed for testcontainers to reach local Docker daemon |
| Buildspec | inline (below) |

```yaml
version: 0.2
phases:
  install:
    commands:
      - rustup update stable
      - cargo --version
  pre_build:
    commands:
      - aws ecr get-login-password | docker login --username AWS --password-stdin $ECR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com
      - cargo fmt -- --check
      - cargo clippy --workspace -- -D warnings
  build:
    commands:
      - cargo test --workspace --locked
      - cargo build --release --workspace --locked
      - docker buildx build --platform linux/arm64 -t $ECR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/drafthouse/auth:$CODEBUILD_RESOLVED_SOURCE_VERSION --target auth .
      - docker buildx build --platform linux/arm64 -t $ECR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/drafthouse/documents:$CODEBUILD_RESOLVED_SOURCE_VERSION --target documents .
      - docker buildx build --platform linux/arm64 -t $ECR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/drafthouse/collab:$CODEBUILD_RESOLVED_SOURCE_VERSION --target collab .
      - docker buildx build --platform linux/arm64 -t $ECR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/drafthouse/export-worker:$CODEBUILD_RESOLVED_SOURCE_VERSION --target export-worker .
      - docker buildx build --platform linux/arm64 -t $ECR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/drafthouse/migrate-pg:$CODEBUILD_RESOLVED_SOURCE_VERSION --target migrate-pg .
      - docker buildx build --platform linux/arm64 -t $ECR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/drafthouse/migrate-scylla:$CODEBUILD_RESOLVED_SOURCE_VERSION --target migrate-scylla .
  post_build:
    commands:
      - docker push $ECR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/drafthouse/auth:$CODEBUILD_RESOLVED_SOURCE_VERSION
      - docker push $ECR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/drafthouse/documents:$CODEBUILD_RESOLVED_SOURCE_VERSION
      - docker push $ECR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/drafthouse/collab:$CODEBUILD_RESOLVED_SOURCE_VERSION
      - docker push $ECR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/drafthouse/export-worker:$CODEBUILD_RESOLVED_SOURCE_VERSION
      - docker push $ECR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/drafthouse/migrate-pg:$CODEBUILD_RESOLVED_SOURCE_VERSION
      - docker push $ECR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/drafthouse/migrate-scylla:$CODEBUILD_RESOLVED_SOURCE_VERSION
      - printf '{"imageUri":"%s"}' "$ECR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/drafthouse/auth:$CODEBUILD_RESOLVED_SOURCE_VERSION" > auth-imagedef.json
      # repeat for other services
artifacts:
  files:
    - auth-imagedef.json
    - documents-imagedef.json
    - collab-imagedef.json
    - export-worker-imagedef.json
    - migrate-pg-imagedef.json
    - migrate-scylla-imagedef.json
```

**Stage 3 — Frontend Build:**

CodeBuild project: `drafthouse-frontend-build`

| Parameter | Value |
|---|---|
| Environment image | `aws/codebuild/standard:7.0` (Node 20) |
| Compute type | `BUILD_GENERAL1_MEDIUM` (4 vCPU, 7 GB) |
| Cache | pnpm store `/root/.local/share/pnpm/store` |

```yaml
phases:
  install:
    commands:
      - npm install -g pnpm
      - pnpm install --frozen-lockfile
  build:
    commands:
      - pnpm tsc --noEmit
      - pnpm vitest run
      - make gen
      - git diff --exit-code frontend/shared/api/generated  # fail if OpenAPI spec stale
      - pnpm build
      - aws s3 sync dist/ s3://drafthouse-static-prod/ --cache-control "max-age=31536000,immutable" --exclude "*.html"
      - aws s3 sync dist/ s3://drafthouse-static-prod/ --cache-control "no-cache,no-store" --include "*.html" --exclude "*" 
      - aws cloudfront create-invalidation --distribution-id $CF_DISTRIBUTION_ID --paths "/*.html" "/index.html"
```

**Stage 4 — Migrate:**

CodeBuild project: `drafthouse-migrate`

Runs ECS RunTask for `migrate-pg` and `migrate-scylla` (sequential — Postgres first, then ScyllaDB). Waits for both to succeed before proceeding. Fails pipeline if migration fails.

**Stage 5 — Deploy Auth:**

Action: `ECS (Blue/Green)` — rolling update for stateless auth service.

| Parameter | Value |
|---|---|
| Action provider | Amazon ECS |
| Cluster | `drafthouse-prod` |
| Service | `auth-service` |
| Image definitions file | `auth-imagedef.json` |

**Stage 6 — Deploy Documents:**

Same as auth, service `documents-service`.

**Stage 7 — Deploy Collab:**

Same pattern, but slower rollout (minHealthyPercent=80 set in service, not pipeline).

| Parameter | Value |
|---|---|
| Action provider | Amazon ECS |
| Service | `collab-service` |
| Image definitions file | `collab-imagedef.json` |

**Stage 8 — Smoke Tests:**

CodeBuild project: `drafthouse-smoke-tests`

```yaml
phases:
  build:
    commands:
      - curl -sf https://drafthouse.app/auth/health
      - curl -sf https://drafthouse.app/documents/health  
      - curl -sf https://drafthouse.app/collab/health
      # Basic auth flow smoke test
      - python3 smoke_tests/auth_flow.py
```

Failure rolls back pipeline (CodePipeline stops, previous task definition still running).

---

## 15. Observability

### CloudWatch Log Groups

| Log group | Retention | Source |
|---|---|---|
| `/ecs/auth` | 30 days | Auth service tasks |
| `/ecs/documents` | 30 days | Documents service tasks |
| `/ecs/collab` | 30 days | Collab service tasks |
| `/ecs/export-worker` | 30 days | Export worker tasks |
| `/aws/rds/drafthouse/postgresql` | 7 days | RDS slow query log |
| `/aws/vpc/flowlogs` | 7 days | VPC Flow Logs |
| `/aws/waf/drafthouse` | 30 days | WAF access log |
| `/aws/elasticache/redis/slow` | 7 days | Redis slow log |
| `/aws/codebuild/drafthouse-*` | 7 days | Build logs |

**Metric filters on `/ecs/collab`:**

| Filter name | Pattern | Metric name | Namespace |
|---|---|---|---|
| `WSReconnectCount` | `{ $.event = "ws_reconnect" }` | `WSReconnects` | `Drafthouse/Collab` |
| `DocSizeLimitApproach` | `{ $.event = "doc_size_limit_approached" }` | `DocSizeLimitWarnings` | `Drafthouse/Collab` |
| `CatchUnwindTriggered` | `{ $.event = "catch_unwind_triggered" }` | `MaliciousClientOps` | `Drafthouse/Collab` |

### CloudWatch Alarms

| Alarm | Metric | Threshold | Period | Action |
|---|---|---|---|---|
| `RDSCPUHigh` | RDS CPUUtilization | > 80% | 5 min | SNS alert |
| `RDSFreeStorageLow` | RDS FreeStorageSpace | < 50 GB | 5 min | SNS alert (page) |
| `RDSConnectionsHigh` | RDS DatabaseConnections | > 450 | 5 min | SNS alert |
| `RDSProxyPoolExhausted` | RDSProxy ClientConnectionsReceived vs Available | pool > 95% | 1 min | SNS alert (page) |
| `RedisMemoryHigh` | ElastiCache DatabaseMemoryUsagePercentage | > 85% | 5 min | SNS alert |
| `CollabWSConnectionsHigh` | `Drafthouse/Collab/active_ws_connections` | > 120000 | 1 min | SNS alert (scale warning) |
| `ExportDLQNonEmpty` | SQS `drafthouse-export-jobs-dlq.fifo` ApproximateNumberOfMessagesVisible | > 0 | 1 min | SNS alert (page — GDPR) |
| `WAFBlockRateHigh` | WAF BlockedRequests | > 10000/5min | 5 min | SNS alert |
| `ECSCollabTaskCount` | ECS RunningTaskCount for collab | < 8 | 1 min | SNS alert (page) |
| `ScyllaWriteLatencyHigh` | `ScyllaDB/WriteLatency` p99 | > 50ms | 5 min | SNS alert |
| `ALB5xxHigh` | ALB HTTPCode_ELB_5XX_Count | > 100/min | 1 min | SNS alert |
| `CFErrorRateHigh` | CloudFront 5xxErrorRate | > 1% | 5 min | SNS alert |

SNS topic: `drafthouse-ops-alerts` → email + PagerDuty integration (HTTP endpoint subscription).

### CloudWatch Dashboard: `Drafthouse-Production`

Widgets:
1. ALB: RequestCount, TargetResponseTime p50/p99, 4xx/5xx rates
2. ECS task counts per service + CPU/Memory utilization
3. RDS: CPUUtilization, DatabaseConnections, ReadLatency, WriteLatency, FreeStorageSpace
4. RDS Proxy: ClientConnectionsReceived, DatabaseConnectionsCurrentlyBorrowed
5. ElastiCache: CacheHits, CacheMisses, CurrConnections, DatabaseMemoryUsagePercentage
6. ScyllaDB: WriteLatency p99, ReadLatency p99, PendingCompactions (custom metrics)
7. SQS: export queue depth, title-update queue depth, DLQ depths
8. WAF: AllowedRequests, BlockedRequests, CountedRequests
9. CloudFront: Requests, BytesDownloaded, 4xxErrorRate, 5xxErrorRate
10. Custom: active_ws_connections (collab), docs_in_memory (collab), op_broadcast_latency_ms p99

### Amazon Managed Service for Prometheus (AMP)

| Parameter | Value |
|---|---|
| Workspace | `drafthouse-prod` |
| Retention | 150 days |
| Remote write endpoint | `https://aps-workspaces.us-east-1.amazonaws.com/workspaces/{id}/api/v1/remote_write` |

ADOT collector sidecar on each ECS task scrapes `/metrics` (port 9090) and remote-writes to AMP. Collab service exposes all metrics from ARCHITECTURE.md §11 plus new ones:
- `redis_relay_latency_ms` histogram (new — pub/sub relay latency)
- `scylla_wal_write_latency_ms` histogram
- `snapshot_s3_upload_duration_ms` histogram

### Amazon Managed Grafana (AMG)

| Parameter | Value |
|---|---|
| Authentication | AWS IAM Identity Center (SSO) |
| Data sources | AMP, CloudWatch, AWS X-Ray |
| Notification channels | SNS → PagerDuty |

Dashboards: import existing Grafana JSON (from compose.yml Grafana), update data source references to AMP.

### AWS X-Ray Distributed Tracing

| Parameter | Value |
|---|---|
| Sampling rule | 5% baseline, 100% for 5xx responses |
| Reservoir | 50 req/sec guaranteed sampling |
| X-Ray daemon | sidecar container on all ECS tasks (port 2000/udp) |
| Instrumentation | `tracing-actix-web` crate with X-Ray propagation, `aws-xray-sdk-rust` (or manual segment creation) |

Trace propagation flow: CloudFront injects `X-Amzn-Trace-Id` → ALB propagates → ECS task creates subsegments for RDS, Redis, ScyllaDB calls → X-Ray service map shows end-to-end latency breakdown.

---

## 16. IAM Roles

### ECS Task Execution Role: `ecs-task-execution-role`

Used by all ECS services to pull images and ship logs. Attach AWS managed policy `AmazonECSTaskExecutionRolePolicy` plus:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": "secretsmanager:GetSecretValue",
      "Resource": [
        "arn:aws:secretsmanager:us-east-1:ACCOUNT:secret:drafthouse/*"
      ]
    }
  ]
}
```

### ECS Auth Task Role: `ecs-auth-task-role`

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": "sns:Publish",
      "Resource": "arn:aws:sns:us-east-1:ACCOUNT:drafthouse-export-requested"
    },
    {
      "Effect": "Allow",
      "Action": ["xray:PutTraceSegments", "xray:PutTelemetryRecords"],
      "Resource": "*"
    }
  ]
}
```

### ECS Documents Task Role: `ecs-documents-task-role`

```json
{
  "Statement": [
    {
      "Effect": "Allow",
      "Action": "sns:Publish",
      "Resource": "arn:aws:sns:us-east-1:ACCOUNT:drafthouse-title-updated"
    },
    {
      "Effect": "Allow",
      "Action": ["xray:PutTraceSegments", "xray:PutTelemetryRecords"],
      "Resource": "*"
    },
    {
      "Effect": "Allow",
      "Action": "aps:RemoteWrite",
      "Resource": "arn:aws:aps:us-east-1:ACCOUNT:workspace/WORKSPACE_ID"
    }
  ]
}
```

### ECS Collab Task Role: `ecs-collab-task-role`

```json
{
  "Statement": [
    {
      "Effect": "Allow",
      "Action": ["sqs:ReceiveMessage", "sqs:DeleteMessage", "sqs:GetQueueAttributes"],
      "Resource": "arn:aws:sqs:us-east-1:ACCOUNT:drafthouse-collab-title-updates"
    },
    {
      "Effect": "Allow",
      "Action": ["s3:GetObject", "s3:PutObject", "s3:DeleteObject"],
      "Resource": "arn:aws:s3:::drafthouse-snapshots-prod/*"
    },
    {
      "Effect": "Allow",
      "Action": ["xray:PutTraceSegments", "xray:PutTelemetryRecords"],
      "Resource": "*"
    },
    {
      "Effect": "Allow",
      "Action": "aps:RemoteWrite",
      "Resource": "arn:aws:aps:us-east-1:ACCOUNT:workspace/WORKSPACE_ID"
    },
    {
      "Effect": "Allow",
      "Action": "cloudwatch:PutMetricData",
      "Resource": "*",
      "Condition": {
        "StringEquals": { "cloudwatch:namespace": "Drafthouse/Collab" }
      }
    }
  ]
}
```

### ECS Export Worker Task Role: `ecs-export-worker-task-role`

```json
{
  "Statement": [
    {
      "Effect": "Allow",
      "Action": ["sqs:ReceiveMessage", "sqs:DeleteMessage", "sqs:ChangeMessageVisibility", "sqs:GetQueueAttributes"],
      "Resource": "arn:aws:sqs:us-east-1:ACCOUNT:drafthouse-export-jobs.fifo"
    },
    {
      "Effect": "Allow",
      "Action": "s3:GetObject",
      "Resource": "arn:aws:s3:::drafthouse-snapshots-prod/*"
    },
    {
      "Effect": "Allow",
      "Action": ["s3:PutObject", "s3:GetObject"],
      "Resource": "arn:aws:s3:::drafthouse-exports-prod/*"
    }
  ]
}
```

### ScyllaDB Instance Profile: `scylla-instance-profile`

```json
{
  "Statement": [
    {
      "Effect": "Allow",
      "Action": ["s3:PutObject", "s3:PutObjectAcl"],
      "Resource": "arn:aws:s3:::drafthouse-backups-prod/scylla/*"
    },
    {
      "Effect": "Allow",
      "Action": ["ssm:UpdateInstanceInformation", "ssmmessages:*", "ec2messages:*"],
      "Resource": "*"
    },
    {
      "Effect": "Allow",
      "Action": ["cloudwatch:PutMetricData"],
      "Resource": "*",
      "Condition": {
        "StringEquals": { "cloudwatch:namespace": "ScyllaDB" }
      }
    }
  ]
}
```

---

## 17. Security Groups

### `alb-sg`

| Direction | Protocol | Port | Source/Dest | Reason |
|---|---|---|---|---|
| Inbound | TCP | 443 | CloudFront managed prefix list (`com.amazonaws.global.cloudfront.origin-facing`) | HTTPS from CloudFront only |
| Inbound | TCP | 80 | `0.0.0.0/0` | HTTP for redirect to HTTPS |
| Outbound | TCP | 8080 | `ecs-auth-sg` | To auth tasks |
| Outbound | TCP | 8080 | `ecs-documents-sg` | To documents tasks |
| Outbound | TCP | 8080 | `ecs-collab-sg` | To collab tasks |

### `ecs-auth-sg`

| Direction | Protocol | Port | Source/Dest | Reason |
|---|---|---|---|---|
| Inbound | TCP | 8080 | `alb-sg` | Traffic from ALB |
| Outbound | TCP | 5432 | `rds-proxy-sg` | Postgres via RDS Proxy |
| Outbound | TCP | 443 | `0.0.0.0/0` | Resend API, Secrets Manager, SNS (VPC endpoints for Secrets/SNS, else NAT GW) |

### `ecs-documents-sg`

| Direction | Protocol | Port | Source/Dest | Reason |
|---|---|---|---|---|
| Inbound | TCP | 8080 | `alb-sg` | Traffic from ALB |
| Outbound | TCP | 5432 | `rds-proxy-sg` | Postgres via RDS Proxy |
| Outbound | TCP | 6379 | `redis-sg` | Redis (ws_tickets) |
| Outbound | TCP | 443 | `0.0.0.0/0` | Secrets Manager, SNS |

### `ecs-collab-sg`

| Direction | Protocol | Port | Source/Dest | Reason |
|---|---|---|---|---|
| Inbound | TCP | 8080 | `alb-sg` | WS upgrade + data |
| Outbound | TCP | 9042 | `scylla-sg` | ScyllaDB CQL |
| Outbound | TCP | 6379 | `redis-sg` | Redis pub/sub, WS tickets, snapshot cache |
| Outbound | TCP | 443 | `0.0.0.0/0` | S3 (VPC endpoint), Secrets Manager |
| Outbound | UDP | 2000 | `127.0.0.1/32` | X-Ray daemon (localhost) |

### `ecs-export-sg`

| Direction | Protocol | Port | Source/Dest | Reason |
|---|---|---|---|---|
| Inbound | — | — | — | No inbound |
| Outbound | TCP | 5432 | `rds-proxy-sg` | Read replica via RDS Proxy |
| Outbound | TCP | 443 | `0.0.0.0/0` | S3, SQS, Secrets Manager, Resend |

### `rds-proxy-sg`

| Direction | Protocol | Port | Source/Dest | Reason |
|---|---|---|---|---|
| Inbound | TCP | 5432 | `ecs-auth-sg` | Auth tasks |
| Inbound | TCP | 5432 | `ecs-documents-sg` | Documents tasks |
| Inbound | TCP | 5432 | `ecs-export-sg` | Export worker |
| Inbound | TCP | 5432 | `codebuild-sg` | Migrations during deploy |
| Outbound | TCP | 5432 | `rds-sg` | To RDS instances |

### `rds-sg`

| Direction | Protocol | Port | Source/Dest | Reason |
|---|---|---|---|---|
| Inbound | TCP | 5432 | `rds-proxy-sg` | From RDS Proxy only |
| Outbound | — | — | — | None |

### `redis-sg`

| Direction | Protocol | Port | Source/Dest | Reason |
|---|---|---|---|---|
| Inbound | TCP | 6379 | `ecs-collab-sg` | Collab tasks |
| Inbound | TCP | 6379 | `ecs-documents-sg` | Documents tasks (WS tickets) |
| Outbound | — | — | — | None |

### `scylla-sg`

| Direction | Protocol | Port | Source/Dest | Reason |
|---|---|---|---|---|
| Inbound | TCP | 9042 | `ecs-collab-sg` | CQL from collab tasks |
| Inbound | TCP | 7000 | `scylla-sg` | Inter-node gossip (unencrypted) |
| Inbound | TCP | 7001 | `scylla-sg` | Inter-node gossip (TLS) |
| Inbound | TCP | 9180 | `scylla-sg` | Prometheus metrics scrape (CloudWatch agent on same node) |
| Outbound | TCP | 7000 | `scylla-sg` | Inter-node gossip |
| Outbound | TCP | 7001 | `scylla-sg` | Inter-node gossip (TLS) |
| Outbound | TCP | 443 | `0.0.0.0/0` | S3 backup uploads, SSM |

---

## 18. Backup & DR

### PostgreSQL

| Type | Tool | Frequency | Destination | Retention |
|---|---|---|---|---|
| Automated backup | RDS native | Continuous WAL + daily snapshot | S3 (AWS managed) | 7 days |
| Manual snapshot | RDS snapshot | Weekly (before major deploys), via EventBridge + Lambda | S3 (AWS managed) | 35 days |
| Cross-region copy | RDS snapshot copy | Daily, Lambda triggered by EventBridge | `us-west-2` RDS snapshots | 7 days |

RTO for full RDS failure: ~15 min (restore from snapshot + WAL replay). RPO: near-zero (continuous WAL archiving).

### ScyllaDB

| Type | Tool | Frequency | Destination | Retention |
|---|---|---|---|---|
| Hot snapshot | `nodetool snapshot` via SSM | Hourly | `s3://drafthouse-backups-prod/scylla/` | 7 days in Standard-IA, 30 days in Glacier |
| Cold backup | Full SSTable upload | Daily (full) | Same S3 bucket | 30 days |
| Cross-region | S3 CRR | Continuous | `s3://drafthouse-backups-dr/scylla/` (us-west-2) | Same lifecycle |

Restore procedure for ScyllaDB node failure:
1. Launch new `i4i.4xlarge` in same AZ
2. Install ScyllaDB AMI, configure same `scylla.yaml` with existing seeds
3. Node auto-streams data from surviving nodes (RF=3, 2 nodes survive any single failure)
4. If all 3 nodes fail (catastrophic): launch 3 new nodes, download SSTables from S3, run `sstableloader` to load data

RTO for single ScyllaDB node failure: ~10 min (streaming from peers, depends on data volume). RPO: 0 (surviving nodes hold RF=3 copy). RTO for full cluster loss: 2-4 hours (S3 restore + sstableloader). RPO: 1 hour (hourly snapshots).

### Redis

Redis is coordination state, not primary data. On full Redis cluster loss:
- WS tickets: all existing WS connections lose their ticket (already burned at connect time, no impact). New connections will need new tickets via `POST /documents/:id/ws-ticket` (Redis regenerates automatically).
- Editor counts: reset to 0. Collab tasks repopulate as WS connections reconnect.
- Snapshot metadata cache: cold. First load after recovery hits ScyllaDB directly. Warm within 5 min.
- Pub/sub channels: recreated automatically as collab tasks subscribe.

Redis backups: ElastiCache daily snapshot kept 7 days. Only needed if debugging state corruption — not for operational recovery.

### S3 Buckets

All critical buckets have cross-region replication to `us-west-2`. RTO for S3: near-instant (S3 99.999999999% durability, multi-AZ by default). Cross-region replication is for catastrophic us-east-1 region failure.

### DR Runbook (full us-east-1 failure)

1. Update Route 53 A record to point to `us-west-2` CloudFront distribution
2. Restore RDS from cross-region snapshot copy in `us-west-2`
3. Launch ScyllaDB nodes in `us-west-2` using `drafthouse-backups-dr` S3 data
4. Update ECS services in `us-west-2` cluster (pre-staged with same task definitions via ECR replication)
5. Update Secrets Manager in `us-west-2` with new endpoints
6. Validate health checks

Target RTO for full region failure: 2-4 hours. RPO: 1 hour (ScyllaDB). Postgres RPO: ~5 min (cross-region snapshot copy runs hourly, WAL archiving runs continuously but is not cross-region).

---

## 19. Cost Estimate

Monthly, on-demand pricing (us-east-1, 2025 rates). Apply 30-40% discount with 1-year Reserved Instances for RDS, ElastiCache, and EC2.

| Service | Config | Monthly (on-demand) |
|---|---|---|
| ECS Fargate — auth | 3–20 tasks avg ~5, 2 vCPU / 4 GB arm64 | ~$280 |
| ECS Fargate — documents | 3–30 tasks avg ~5, 0.5 vCPU / 1 GB arm64 | ~$65 |
| ECS Fargate — collab | 10–50 tasks avg ~15, 4 vCPU / 16 GB arm64 | ~$3,200 |
| ECS Fargate — export worker | 0–20 tasks avg ~2, 1 vCPU / 4 GB arm64 | ~$60 |
| RDS `db.r7g.2xlarge` Multi-AZ | primary + standby | ~$835 |
| RDS read replicas ×2 | `db.r7g.xlarge` + `db.r7g.large` | ~$630 |
| RDS Proxy | based on vCPU of underlying RDS | ~$86 |
| RDS Storage 500GB gp3 + IOPS | 12000 IOPS provisioned | ~$160 |
| ElastiCache `cache.r7g.xlarge` ×6 | 3 shards, 1 replica each | ~$1,340 |
| ScyllaDB `i4i.4xlarge` ×3 | On-demand | ~$3,220 |
| ALB | ~200M requests/month | ~$200 |
| CloudFront | 270M requests/month + data transfer | ~$350 |
| WAF | 270M requests × $0.60/million + rule eval | ~$200 |
| S3 | 10TB storage + requests | ~$250 |
| NAT Gateways ×3 | ~5TB/month cross-AZ data | ~$450 |
| VPC Interface Endpoints ×8 ×3 AZs | $0.01/hr each | ~$520 |
| SNS + SQS | millions of messages | ~$15 |
| CodeBuild | Rust builds, ~10 builds/day | ~$150 |
| Secrets Manager | ~15 secrets + API calls | ~$50 |
| CloudWatch + AMP + AMG | logs, metrics, dashboards | ~$350 |
| X-Ray | 5% sample of 270M requests | ~$80 |
| Data transfer (inter-AZ, egress) | estimate | ~$300 |
| ECR | storage + pulls | ~$30 |
| Route 53 | hosted zone + queries | ~$15 |
| **Total on-demand** | | **~$12,600/month** |
| **Total with 1yr RIs** | ~35% reduction on RDS/ElastiCache/EC2 | **~$9,500/month** |

Biggest cost levers:
1. ScyllaDB (`i4i.4xlarge` ×3 = $3,220) — consider ScyllaDB i3.2xlarge if data fits
2. Collab ECS tasks ($3,200) — optimize Yrs memory usage to reduce task size
3. ElastiCache ($1,340) — could reduce to `cache.r7g.large` if Redis memory usage stays under 13GB per shard

---

## 20. Migration Sequence

Order matters — data layer before compute layer.

### Phase 1: Infrastructure (no traffic)

1. VPC, subnets, route tables, NAT GWs, VPC endpoints
2. Security groups (all)
3. RDS PostgreSQL + parameter group + read replicas
4. RDS Proxy (primary + read replica)
5. ElastiCache Redis cluster
6. ScyllaDB EC2 instances + `scylla.yaml` config
7. S3 buckets (all) + lifecycle rules + replication
8. ECR repositories + lifecycle policies
9. Secrets Manager secrets
10. SNS topics + SQS queues + DLQs
11. ACM certificates (DNS validation)
12. Route 53 hosted zone (keep old DNS until cutover)

### Phase 2: Build Pipeline

13. ECR images: build + push all service images
14. CodeBuild projects configured
15. CodePipeline pipeline configured but paused

### Phase 3: Data Migration

16. Run Postgres migrations against new RDS (same migration files)
17. Run ScyllaDB migrations against new cluster
18. If migrating from VPS with live data: `pg_dump` from VPS Postgres → restore to RDS. ScyllaDB: `nodetool snapshot` on VPS → `sstableloader` to AWS ScyllaDB cluster.

### Phase 4: Compute Layer

19. ECS cluster `drafthouse-prod` created
20. Task definitions registered (auth, documents, collab, export-worker)
21. ECS services deployed with `desired_count=1` (test only)
22. ALB + target groups created
23. CloudFront distribution created (points to ALB)
24. WAF Web ACL created + attached to CloudFront

### Phase 5: Validation

25. Smoke tests against CloudFront distribution (not in Route 53 yet)
26. Load test via CloudFront URL: k6 with 50 concurrent editors, verify p99 < 100ms
27. Scale ECS services to production desired counts

### Phase 6: Cutover

28. Update Route 53 A record: `drafthouse.app` → new CloudFront distribution (TTL 60s during cutover)
29. Monitor: ALB 5xx rate, ECS task health, RDS connections, Redis memory
30. Keep VPS running for 24hrs (instant rollback: revert Route 53 TTL)
31. Decommission VPS after 48hrs stable

### Phase 7: Post-Cutover

32. Enable CodePipeline (set to active)
33. Enable EventBridge schedules (ScyllaDB backup cron, pg_dump cron)
34. Configure AMG dashboards
35. Configure PagerDuty integration with CloudWatch alarms
36. Verify GDPR export flow end-to-end
37. Reserved Instance purchases (commit after 1 week of stable traffic)
