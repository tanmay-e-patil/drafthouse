//! Regression tests for the documents/auth audit (docs/BUG_AUDIT.md #4, #7,
//! #16, #30, #31, #32) against a real, disposable PostgreSQL container.
//!
//! Each `regression_NN_...` test asserts the REQUIRED behavior for audit
//! finding #NN and is intentionally red while the bug is unfixed.

use chrono::Utc;
use collab_core::{AwarenessPeer, DocStore, room::get_or_create_room};
use dal::postgres_txs::SqlxPostGresDescriptor;
use dal::*;
use dashmap::DashMap;
use kernel::*;
use scylla::frame::value::CqlTimestamp;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use testcontainers_modules::{
    postgres::Postgres,
    scylladb::ScyllaDB,
    testcontainers::{ContainerAsync, runners::AsyncRunner},
};
use tokio::sync::Barrier;
use utils::errors::{NanoServiceError, NanoServiceErrorStatus};
use uuid::Uuid;

struct TestEnv {
    pool: PgPool,
    _container: ContainerAsync<Postgres>,
}

impl TestEnv {
    async fn new() -> Self {
        let container = Postgres::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
        let pool = PgPool::connect(&url).await.unwrap();
        sqlx::migrate!("../../../migrations/postgres")
            .run(&pool)
            .await
            .unwrap();
        Self {
            pool,
            _container: container,
        }
    }
    fn dal(&self) -> SqlxPostGresDescriptor {
        SqlxPostGresDescriptor {
            pool: self.pool.clone(),
        }
    }
}

async fn user(dal: &SqlxPostGresDescriptor) -> Uuid {
    let hash = auth_core::password::hash_password("original-password").unwrap();
    sqlx::query_scalar("INSERT INTO users (email,password_hash,email_verified_at,welcome_doc_created) VALUES ($1,$2,now(),true) RETURNING id")
        .bind(format!("{}@regression.invalid", Uuid::new_v4()))
        .bind(hash)
        .fetch_one(&dal.pool)
        .await
        .unwrap()
}
async fn document(dal: &SqlxPostGresDescriptor, owner: Uuid) -> Document {
    documents_core::create_document(dal, owner, "regression")
        .await
        .unwrap()
}

/// REG-04 (#4): a delayed projection may never overwrite content derived
/// from a newer authoritative CRDT revision.
#[tokio::test]
async fn regression_04_stale_save_cannot_overwrite_newer_text() {
    let env = TestEnv::new().await;
    let dal = env.dal();
    let owner = user(&dal).await;
    let doc = document(&dal, owner).await;

    assert!(
        dal.project_document_content(doc.id, "new text".into(), 2)
            .await
            .unwrap()
    );
    assert!(
        !dal.project_document_content(doc.id, "old text".into(), 1)
            .await
            .unwrap()
    );

    let (content, revision): (Option<String>, i64) =
        sqlx::query_as("SELECT content, content_revision FROM documents WHERE id = $1")
            .bind(doc.id)
            .fetch_one(&dal.pool)
            .await
            .unwrap();
    assert_eq!(content.as_deref(), Some("new text"));
    assert_eq!(revision, 2);
}

/// REG-07 (#7): deleting a document or an account must also tear down its
/// live collaboration rooms (production wiring: `init_doc_store` + event
/// subscription, as done for `TitleUpdated`).
#[tokio::test]
async fn regression_07_deletion_tears_down_live_rooms() {
    let env = TestEnv::new().await;
    let dal = env.dal();
    let owner = user(&dal).await;
    let doc = document(&dal, owner).await;
    // This test initializes the process-wide DocStore (OnceLock), so it must
    // remain the only test in this binary relying on it.
    let store = Arc::new(DocStore::new());
    collab_core::init_doc_store(store.clone());
    let room = get_or_create_room(&store, doc.id);
    room.add_connection();

    documents_core::delete_document(&dal, doc.id, owner)
        .await
        .unwrap();
    assert!(dal.get_document_by_id(doc.id).await.unwrap().is_none());
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !store.contains_key(&doc.id),
        "deleted document's room must be removed"
    );

    let second = document(&dal, owner).await;
    get_or_create_room(&store, second.id).add_connection();
    auth_core::me::delete_account(&dal, owner, "original-password")
        .await
        .unwrap();
    assert!(dal.get_user_by_id(owner).await.unwrap().is_none());
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !store.contains_key(&second.id),
        "deleted account's rooms must be removed"
    );
}

/// REG-16 (#16, server guard): editor members may edit content but must not
/// rename the document — title changes are owner-only. This test guards the
/// (already correct) server policy while the frontend is fixed to match it.
#[tokio::test]
async fn regression_16_editor_member_cannot_rename_document() {
    let env = TestEnv::new().await;
    let dal = env.dal();
    let owner = user(&dal).await;
    let editor = user(&dal).await;
    let doc = document(&dal, owner).await;
    sqlx::query("INSERT INTO document_members (doc_id,user_id,role) VALUES ($1,$2,'editor')")
        .bind(doc.id)
        .bind(editor)
        .execute(&dal.pool)
        .await
        .unwrap();
    documents_core::ensure_document_editor_access(&dal, doc.id, editor)
        .await
        .unwrap();
    let error = documents_core::update_document(
        &dal,
        doc.id,
        editor,
        &UpdateDocumentRequest {
            title: Some("rename".into()),
            is_public: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.status, NanoServiceErrorStatus::Forbidden);
}

/// REG-30 (#30): pagination must return every document exactly once, even
/// when page-boundary rows share an `updated_at` timestamp.
#[tokio::test]
async fn regression_30_pagination_returns_all_documents_with_equal_timestamps() {
    let env = TestEnv::new().await;
    let dal = env.dal();
    let owner = user(&dal).await;
    for _ in 0..3 {
        document(&dal, owner).await;
    }
    sqlx::query("UPDATE documents SET updated_at = '2026-01-01T00:00:00Z' WHERE owner_id = $1")
        .bind(owner)
        .execute(&dal.pool)
        .await
        .unwrap();
    let first = documents_core::list_documents(&dal, owner, None, Some(2))
        .await
        .unwrap();
    assert!(first.has_more);
    assert_eq!(first.data.len(), 2);
    let second = documents_core::list_documents(&dal, owner, first.next_cursor, Some(2))
        .await
        .unwrap();
    assert_eq!(second.data.len(), 1, "the third document must be returned");
    let mut seen: Vec<Uuid> = first.data.iter().map(|d| d.id).collect();
    seen.extend(second.data.iter().map(|d| d.id));
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 3);
}

async fn reset_token(dal: &SqlxPostGresDescriptor, owner: Uuid) -> String {
    let raw = Uuid::new_v4().to_string();
    dal.create_password_reset_token(NewPasswordResetToken {
        user_id: owner,
        token_hash: auth_core::token::hash_token(&raw),
        expires_at: Utc::now() + chrono::Duration::minutes(10),
    })
    .await
    .unwrap();
    raw
}

/// REG-31 (#31): password reset enforces the same minimum length as
/// registration and password change.
#[tokio::test]
async fn regression_31_reset_rejects_short_password() {
    let env = TestEnv::new().await;
    let dal = env.dal();
    let owner = user(&dal).await;
    let token = reset_token(&dal, owner).await;
    let error = auth_core::password_reset::reset_password(&dal, &token, "")
        .await
        .unwrap_err();
    assert_eq!(error.status, NanoServiceErrorStatus::BadRequest);
    let stored = dal.get_user_by_id(owner).await.unwrap().unwrap();
    assert!(
        auth_core::password::verify_password("original-password", &stored.password_hash).unwrap()
    );
}

// A barrier pauses only after the real database read. All mutations remain
// real SQL; it makes the existing read/consume race deterministic.
struct RacingDal {
    inner: SqlxPostGresDescriptor,
    both_have_read: Barrier,
}
impl GetRefreshTokenByHash for RacingDal {
    async fn get_refresh_token_by_hash(
        &self,
        hash: String,
    ) -> Result<Option<RefreshToken>, NanoServiceError> {
        let token = self.inner.get_refresh_token_by_hash(hash).await?;
        self.both_have_read.wait().await;
        Ok(token)
    }
}
impl GetPasswordResetToken for RacingDal {
    async fn get_password_reset_token(
        &self,
        hash: String,
    ) -> Result<Option<PasswordResetToken>, NanoServiceError> {
        let token = self.inner.get_password_reset_token(hash).await?;
        self.both_have_read.wait().await;
        Ok(token)
    }
}
impl GetUserById for RacingDal {
    async fn get_user_by_id(&self, id: Uuid) -> Result<Option<User>, NanoServiceError> {
        self.inner.get_user_by_id(id).await
    }
}
impl DeleteRefreshToken for RacingDal {
    async fn delete_refresh_token(&self, hash: String) -> Result<(), NanoServiceError> {
        self.inner.delete_refresh_token(hash).await
    }
}
impl CreateRefreshToken for RacingDal {
    async fn create_refresh_token(
        &self,
        token: NewRefreshToken,
    ) -> Result<RefreshToken, NanoServiceError> {
        self.inner.create_refresh_token(token).await
    }
}
impl MarkPasswordResetTokenUsed for RacingDal {
    async fn mark_password_reset_token_used(&self, hash: String) -> Result<(), NanoServiceError> {
        self.inner.mark_password_reset_token_used(hash).await
    }
}
impl UpdateUserPassword for RacingDal {
    async fn update_user_password(&self, id: Uuid, hash: String) -> Result<(), NanoServiceError> {
        self.inner.update_user_password(id, hash).await
    }
}
impl DeleteAllRefreshTokensForUser for RacingDal {
    async fn delete_all_refresh_tokens_for_user(&self, id: Uuid) -> Result<(), NanoServiceError> {
        self.inner.delete_all_refresh_tokens_for_user(id).await
    }
}

/// REG-32 (#32): a refresh token is consumed exactly once, even under
/// concurrent use.
#[tokio::test]
async fn regression_32_refresh_token_consumed_exactly_once() {
    let env = TestEnv::new().await;
    let dal = env.dal();
    let owner = user(&dal).await;
    let raw = Uuid::new_v4().to_string();
    dal.create_refresh_token(NewRefreshToken {
        user_id: owner,
        token_hash: auth_core::token::hash_token(&raw),
        expires_at: Utc::now() + chrono::Duration::days(1),
    })
    .await
    .unwrap();
    let dal = RacingDal {
        inner: dal,
        both_have_read: Barrier::new(2),
    };
    let (a, b) = tokio::join!(
        auth_core::login::refresh_access_token(&dal, &raw),
        auth_core::login::refresh_access_token(&dal, &raw)
    );
    let successes = [a.is_ok(), b.is_ok()].iter().filter(|r| **r).count();
    assert_eq!(successes, 1, "exactly one refresh must succeed");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1")
        .bind(owner)
        .fetch_one(&dal.inner.pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "exactly one replacement token must remain");
}

/// REG-32 (#32): a password reset token is consumed exactly once.
#[tokio::test]
async fn regression_32_password_reset_token_consumed_exactly_once() {
    let env = TestEnv::new().await;
    let dal = env.dal();
    let owner = user(&dal).await;
    let raw = reset_token(&dal, owner).await;
    let dal = RacingDal {
        inner: dal,
        both_have_read: Barrier::new(2),
    };
    let (a, b) = tokio::join!(
        auth_core::password_reset::reset_password(&dal, &raw, "password-one"),
        auth_core::password_reset::reset_password(&dal, &raw, "password-two")
    );
    let successes = [a.is_ok(), b.is_ok()].iter().filter(|r| **r).count();
    assert_eq!(successes, 1, "exactly one reset must succeed");
    assert!(
        dal.inner
            .get_password_reset_token(auth_core::token::hash_token(&raw))
            .await
            .unwrap()
            .unwrap()
            .used_at
            .is_some()
    );
    let winner = if a.is_ok() {
        "password-one"
    } else {
        "password-two"
    };
    let stored = dal.inner.get_user_by_id(owner).await.unwrap().unwrap();
    assert!(auth_core::password::verify_password(winner, &stored.password_hash).unwrap());
}

/// Guard (not tied to one audit finding): invite-link `max_uses` must be
/// enforced under concurrent acceptance — the `SELECT ... FOR UPDATE` in
/// `accept_invite_link` serializes the check-and-increment. No existing
/// integration test exercises `max_uses` at all.
#[tokio::test]
async fn guard_invite_link_max_uses_enforced_under_concurrency() {
    let env = TestEnv::new().await;
    let dal = env.dal();
    let owner = user(&dal).await;
    let doc = document(&dal, owner).await;
    documents_core::create_invite_link(
        &dal,
        doc.id,
        owner,
        &CreateInviteLinkRequest {
            role: MemberRole::Editor,
            max_uses: Some(1),
            expires_at: None,
        },
    )
    .await
    .unwrap();
    let token: String = sqlx::query_scalar("SELECT token FROM invite_links WHERE doc_id = $1")
        .bind(doc.id)
        .fetch_one(&dal.pool)
        .await
        .unwrap();

    let (a, b) = tokio::join!(
        documents_core::accept_invite(&dal, &token, user(&dal).await),
        documents_core::accept_invite(&dal, &token, user(&dal).await)
    );
    let successes = [a.is_ok(), b.is_ok()].iter().filter(|r| **r).count();
    assert_eq!(successes, 1, "exactly one concurrent accept must succeed");
    let use_count: i32 = sqlx::query_scalar("SELECT use_count FROM invite_links WHERE token = $1")
        .bind(&token)
        .fetch_one(&dal.pool)
        .await
        .unwrap();
    assert_eq!(use_count, 1);
}

/// Guard: the presence endpoint reports only recent peers, deduplicated per
/// user (the existing integration test only covers the empty-room case).
#[tokio::test]
async fn guard_presence_endpoint_lists_recent_deduplicated_peers_only() {
    use actix_web::{App, http::StatusCode, test, web};

    let env = TestEnv::new().await;
    let dal = env.dal();
    let owner = user(&dal).await;
    let doc = document(&dal, owner).await;

    let doc_store: web::Data<DocStore> = web::Data::new(DashMap::new());
    let room = get_or_create_room(&doc_store, doc.id);
    let now_ms = Utc::now().timestamp_millis();
    let peer = |user_id: Option<Uuid>, name: &str, last_active_ms: i64| AwarenessPeer {
        user_id,
        name: name.to_string(),
        color: "#123456".to_string(),
        last_active_ms,
    };
    // Two sessions of the same user (must collapse to one entry)...
    room.apply_awareness_update(1, vec![(101, 1, Some(peer(Some(owner), "owner", now_ms)))]);
    room.apply_awareness_update(
        1,
        vec![(102, 1, Some(peer(Some(owner), "owner", now_ms - 60_000)))],
    );
    // ...and a stale anonymous peer past the 5-minute cutoff.
    room.apply_awareness_update(
        1,
        vec![(103, 1, Some(peer(None, "anon", now_ms - 6 * 60_000)))],
    );

    let pool = env.pool.clone();
    let app = test::init_service(App::new().configure(move |cfg| {
        documents_networking::routes::configure(
            cfg,
            web::Data::new(SqlxPostGresDescriptor { pool: pool.clone() }),
            doc_store.clone(),
        )
    }))
    .await;

    let token = auth_core::jwt::create_jwt(owner, "owner@regression.invalid", true).unwrap();
    let req = test::TestRequest::get()
        .uri(&format!("/documents/{}/presence", doc.id))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    let data = body["data"].as_array().unwrap();
    assert_eq!(
        data.len(),
        1,
        "stale peers filtered, same-user sessions deduplicated"
    );
    assert_eq!(data[0]["user_id"].as_str().unwrap(), owner.to_string());
    assert_eq!(data[0]["name"].as_str().unwrap(), "owner");
}

struct ScyllaEnv {
    dal: dal::ScyllaDescriptor,
    _container: ContainerAsync<ScyllaDB>,
}

impl ScyllaEnv {
    async fn new() -> Self {
        let container = ScyllaDB::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(9042).await.unwrap();
        let session = scylla::SessionBuilder::new()
            .known_node(format!("127.0.0.1:{port}"))
            .build()
            .await
            .unwrap();
        session
            .query_unpaged(
                "CREATE KEYSPACE IF NOT EXISTS drafthouse \
                 WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1}",
                &[],
            )
            .await
            .unwrap();
        for cql in [
            include_str!("../../../../migrations/scylla/0002_create_ops.cql"),
            include_str!("../../../../migrations/scylla/0003_create_snapshots.cql"),
            include_str!("../../../../migrations/scylla/0004_create_ordered_collab_storage.cql"),
        ] {
            for stmt in cql
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty() && !s.starts_with("--"))
            {
                session.query_unpaged(stmt, &[]).await.unwrap();
            }
        }
        Self {
            dal: dal::ScyllaDescriptor {
                session: Arc::new(session),
                keyspace: "drafthouse".to_string(),
            },
            _container: container,
        }
    }

    /// Register this descriptor as the process-wide collab DAL so deletion
    /// events purge this Scylla (production wiring mirrors ingress).
    fn register_globally(&self) {
        collab_core::init_collab_dal(Arc::new(self.dal.clone()));
    }

    async fn seed_collab_data(&self, doc_id: Uuid, marker: &str) {
        // Bypass the DAL write path (see audit #33) so this test targets
        // deletion-purge semantics, not the timestamp serialization bug.
        self.dal
            .session
            .query_unpaged(
                format!(
                    "INSERT INTO {}.ops (doc_id, created_at, op_id, client_id, data) \
                     VALUES (?, ?, ?, ?, ?)",
                    self.dal.keyspace
                ),
                (
                    doc_id,
                    CqlTimestamp(Utc::now().timestamp_millis()),
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    format!("op-{marker}").into_bytes(),
                ),
            )
            .await
            .unwrap();
        self.dal
            .session
            .query_unpaged(
                format!(
                    "INSERT INTO {}.snapshots (doc_id, version, data, checksum, taken_at) \
                     VALUES (?, ?, ?, ?, ?)",
                    self.dal.keyspace
                ),
                (
                    doc_id,
                    1,
                    format!("snapshot-{marker}").into_bytes(),
                    format!("checksum-{marker}"),
                    CqlTimestamp(Utc::now().timestamp_millis()),
                ),
            )
            .await
            .unwrap();
    }

    async fn remaining_rows(&self, doc_id: Uuid) -> usize {
        let count = |table: &str| {
            format!(
                "SELECT doc_id FROM {}.{} WHERE doc_id = ?",
                self.dal.keyspace, table
            )
        };
        let ops = self
            .dal
            .session
            .query_unpaged(count("ops"), (doc_id,))
            .await
            .unwrap()
            .into_rows_result()
            .unwrap()
            .rows::<(Uuid,)>()
            .unwrap()
            .count();
        let snapshots = self
            .dal
            .session
            .query_unpaged(count("snapshots"), (doc_id,))
            .await
            .unwrap()
            .into_rows_result()
            .unwrap()
            .rows::<(Uuid,)>()
            .unwrap()
            .count();
        ops + snapshots
    }
}

/// REG-07 (#7, erasure): deleting a document or an account must purge its
/// ScyllaDB WAL ops and snapshots, not only the Postgres rows.
#[tokio::test]
async fn regression_07_deletion_purges_scylla_data() {
    let env = TestEnv::new().await;
    let dal = env.dal();
    let scylla = ScyllaEnv::new().await;
    scylla.register_globally();
    let owner = user(&dal).await;

    let doc = document(&dal, owner).await;
    scylla.seed_collab_data(doc.id, "doc").await;
    documents_core::delete_document(&dal, doc.id, owner)
        .await
        .unwrap();
    wait_until_purged(&scylla, doc.id).await;

    let second = document(&dal, owner).await;
    scylla.seed_collab_data(second.id, "account").await;
    auth_core::me::delete_account(&dal, owner, "original-password")
        .await
        .unwrap();
    wait_until_purged(&scylla, second.id).await;
}

/// Deletion propagates through an async in-process event, so poll for the
/// required end state instead of asserting on a racy fixed delay.
async fn wait_until_purged(scylla: &ScyllaEnv, doc_id: Uuid) {
    for _ in 0..100 {
        if scylla.remaining_rows(doc_id).await == 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        scylla.remaining_rows(doc_id).await,
        0,
        "collab data must be purged after deletion"
    );
}
