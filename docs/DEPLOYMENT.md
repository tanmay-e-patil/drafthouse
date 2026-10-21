# Dokploy deployment

Keep the existing frontend Application and backend Compose deployment, with one
public HTTPS domain. Dokploy routes `/` to the frontend and `/api` to the backend.
The frontend image is still built from `frontend/Dockerfile`; the backend uses
`compose.dokploy.yml`. The existing CI webhooks continue deploying both services.

## Existing PostgreSQL container

The production Compose file does not create or manage PostgreSQL. Before deploying:

1. Create a dedicated `drafthouse` database and role in the existing PostgreSQL
   container, or use the existing Drafthouse database there. The role must be able
   to run the migrations (including creating tables in the target schema).
2. Find a Docker network shared with that container. For Dokploy-managed databases
   this is usually `dokploy-network`. Use the database's internal Docker DNS name
   and port, rather than `localhost` or its published host port. The database must
   be running and accepting connections before deployment.
3. Set these backend Compose environment variables in Dokploy:

   ```dotenv
   APP_ORIGIN=https://draft.example.com
   DATABASE_URL=postgres://drafthouse:URL_ENCODED_PASSWORD@existing-postgres:5432/drafthouse
   POSTGRES_NETWORK=dokploy-network
   RESEND_API_KEY=<your existing Resend key>
   ```

   Replace the example values with the existing database connection details.
   Percent-encode special characters in the URL's username and password.

Both `migrate-pg` and `app` join `POSTGRES_NETWORK` and use the same `DATABASE_URL`.
The backend starts after both database migration services succeed. ScyllaDB
continues using its existing volume and the Compose default network.

If the current app's data is in the old Compose-managed Postgres container, back
it up and restore it into the target database before switching the URL. Schema
migrations do **not** copy users, documents, or tokens. Pause writes during the
final backup/restore and cutover. Keep the old database and `pg_data` volume until
the new deployment is verified; do not run `docker compose down -v`. Switching
Dokploy projects also changes Compose volume names by default, so keep this
backend deployment in its existing project to retain the ScyllaDB data.

## One domain

Point the domain's DNS at the Dokploy server, then configure these entries in
Dokploy's Domains settings. Use the same host and HTTPS certificate for both:

- Frontend Application: host `draft.example.com`, path `/`, container port `3000`,
  Strip Path disabled, Internal Path empty.
- Backend Compose service `app`: host `draft.example.com`, path `/api`, container
  port `8080`, **Strip Path enabled**, Internal Path empty.

For example, `/api/auth/login` reaches the backend as `/auth/login`, and
`/api/collab/<document-id>` reaches it as `/collab/<document-id>`. The more specific
`/api` route must take precedence over the frontend's `/` route. The backend port
is internal; no host port mapping is required.

Dokploy adds the routing labels and proxy network when deploying. In Preview
Compose, verify that `app` retains its `default` and `postgres` networks as well
as access to the proxy network, so it can reach both ScyllaDB and PostgreSQL.
Redeploy the backend after changing its domain settings.

Remove the old frontend build variables `VITE_API_URL`, `VITE_WS_URL`, and
`VITE_APP_ORIGIN`, then rebuild/redeploy the frontend. Production builds default
to `/api`, derive `wss://draft.example.com/api` for WebSockets on HTTPS, and use the
current origin for share links. Vite variables are baked into the client build;
changing runtime variables alone does not update an existing bundle.

Local development still defaults to `http://localhost:8080` and
`ws://localhost:8080`. Explicit Vite URL overrides remain supported.

## Verify the cutover

1. Confirm the Postgres migration container exits successfully and `app` starts.
2. Open `https://draft.example.com`, register or log in, and reload to verify the
   refresh cookie works. Existing sessions on the old domain may need a new login.
3. In browser Network tools, confirm API requests go to `/api/...` and return API
   responses rather than frontend HTML.
4. Open the same document in two sessions. Confirm the WebSocket request to
   `wss://draft.example.com/api/collab/<document-id>` upgrades with status `101`
   and edits sync between both sessions.
5. Check an email verification/reset link and a document share link use the new
   domain. Verify previously stored users and documents if data was moved.
6. Remove the old domain routes once the new domain passes these checks.

Dokploy references: [Domains](https://docs.dokploy.com/docs/core/domains) and
[Compose domains and networking](https://docs.dokploy.com/docs/core/docker-compose/domains).
