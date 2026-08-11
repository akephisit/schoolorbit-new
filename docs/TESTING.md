# Testing

Use focused tests while implementing, then run every applicable section below. Commands that need databases, credentials, browsers, or deployed services must be reported explicitly when the required environment is unavailable; do not present an unrun check as passing.

## Reporting Verification

Record:

- the exact command that ran;
- whether it passed, failed, or was skipped;
- the relevant failure when it did not pass;
- the missing environment variable or external dependency when it could not run.

Do not replace a failed check by disabling it or by running a narrower command that misses the failure.

## Every Change

From the repository root:

```bash
git diff --check
git status --short
```

Review the final diff and run focused tests for the behavior changed before broad checks.

## Backend School

From `backend-school`:

```bash
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
```

Run focused unit or integration tests for changed modules as well. For API-contract work:

```bash
cargo test api_contract::tests -- --nocapture
```

The static architecture suite owns backend-only boundaries such as tenant request context, thin handlers, service/database separation, permission invalidation, structured logging, and realtime identity.

### School session authentication

Session changes require the schema, repository, service, HTTP/middleware, and realtime boundaries—not only a login happy path. From the repository root, use the disposable local PostgreSQL runner:

```bash
./scripts/test_backend_school.sh modules::auth::session_schema_tests -- --nocapture
./scripts/test_backend_school.sh modules::auth::session_repository_tests -- --nocapture
./scripts/test_backend_school.sh modules::auth::session_service_tests -- --nocapture
./scripts/test_backend_school.sh modules::auth::session_http_tests -- --nocapture
./scripts/test_backend_school.sh modules::academic::websockets::tests -- --nocapture
```

From `frontend-school`, verify the browser security boundary and client state:

```bash
node --test tests/static/session-auth-contract.test.mjs \
  tests/static/account-security.test.mjs \
  tests/static/auth-session-state.test.mjs
E2E_SESSION_USERNAME='dedicated-disposable-account' \
E2E_SESSION_PASSWORD='provided-at-runtime' \
npx playwright test --list tests/e2e/login.spec.ts tests/e2e/session-security.spec.ts
```

The destructive `session-security.spec.ts` account must be dedicated and disposable because the suite intentionally revokes a selected browser and then logs out every session. Never fall back to `SMOKE_*`, normal `E2E_USERNAME`/`E2E_PASSWORD`, or an operator account for that file.

## Backend Admin

From `backend-admin`:

```bash
cargo fmt --all -- --check
cargo test
cargo check
```

Use focused test filters first when changing a handler, client, or service.

## Frontend School

From `frontend-school`:

```bash
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:static
```

During implementation, run the relevant static file directly:

```bash
node --test tests/static/<area>.test.mjs
```

## Frontend Admin

From `frontend-admin`:

```bash
npm run lint
npm run check
npm run build
```

## Installer and Production Topology

When the VPS installer, canonical Compose runtime, Nginx templates, deployment workflows, or their durable documentation changes, run from the repository root:

```bash
shellcheck scripts/schoolorbit-installer scripts/render_nginx_config.sh \
  scripts/lib/schoolorbit-installer/*.sh \
  scripts/lib/schoolorbit-installer/remote/*.sh
shfmt -d -i 4 -ci scripts/schoolorbit-installer scripts/render_nginx_config.sh \
  scripts/lib/schoolorbit-installer/*.sh \
  scripts/lib/schoolorbit-installer/remote/*.sh
bats scripts/tests/installer
node --test frontend-school/tests/static/deployment-installer.test.mjs
env $(grep -v '^#' scripts/tests/installer/fixtures/runtime.env | xargs) \
  podman-compose -f podman-compose.yml --dry-run up -d >/dev/null
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
```

The deployment static guard renders a proxy template into a temporary target, rejects invalid domains without replacing existing output, enforces the single production Compose owner, and confirms backend workflows verify the selected origin rather than the public hostname. Report an unavailable Bats, Podman Compose, or Docker dependency as unrun; do not replace its check with a narrower command.

The guard also owns CI cache policy: backend-admin and backend-school use distinct BuildKit scopes, while API and permission contract jobs share a dependency-oriented backend-school Rust cache. Pull requests are restore-only, and only trusted `main` runs may save it. A cache miss must execute the complete workflow rather than bypassing a gate. API Contract keeps artifact generation/offline export, backend validation, and frontend validation in independent jobs without `needs`; the static guard owns this division and its single-writer Rust cache policy.

The installer Bats directory includes focused Cockpit coverage:

- `cockpit_provider.bats` exercises Tunnel creation/adoption, ingress, connector IP, CNAME drift, memory-only token handling, and management rollback without deleting Tunnels;
- `cockpit_remote.bats` executes the remote configurator against an isolated filesystem and checks loopback-only listening, root prohibition, mode-`0600` token storage, pinned amd64/arm64 cloudflared artifacts, and idempotency;
- `vps.bats` verifies separate SSH stdin streams for the tracked script and secret JSON plus a fresh verification session;
- `orchestration.bats` verifies phase ordering, dry-run mutation absence, publish journaling, resume, standalone rollback, and full migration rollback.

Do not replace these focused files with source-only assertions. The deployment static guard supplements them by preventing Compose, firewall, installer-entry-point, and canonical-document drift.

## Permission Contract

`contracts/permissions.json` is the handwritten permission source. The registries and lock are generated files; do not edit generated files directly.

After a permission definition changes, from `frontend-school`:

```bash
npm run generate:permissions
npm run check:permissions
npm run test:permissions
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Commit the contract, `contracts/permissions.lock.json`, backend registry, frontend registry, migration when DB permission data changes, and focused authorization tests together.

## API Contract

Rust DTOs and OpenAPI annotations own the wire contract. The tracked output is `contracts/openapi/school-api.json`; generated TypeScript lives under `frontend-school/src/lib/api/generated/`. These are generated files; do not edit generated files directly.

After a documented DTO or endpoint changes, from `frontend-school`:

```bash
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Generation must work offline without database credentials or a running backend.

## Database and Migration Tests

Never edit an applied migration. Add a new sequential file and test it against isolated state.

Routine backend-school database tests run on the developer's computer. From the repository root:

```bash
# Complete backend-school binary suite; PostgreSQL runs in Docker Desktop on this computer.
./scripts/test_backend_school.sh

# Focused database-backed test.
./scripts/test_backend_school.sh \
  modules::auth::session_repository_tests -- --nocapture
```

Docker Desktop WSL integration must be active when the repository runs in WSL. The runner accepts only a local Docker endpoint, runs Cargo and its compilation cache on the computer, creates no persistent database volume, and removes its exact PostgreSQL container after success, failure, `INT`, `TERM`, or `HUP`. It replaces any inherited `TEST_DATABASE_URL` only for the Cargo child and never uses `DATABASE_URL`. Direct Cargo against a persistent Neon URL is not the routine test recipe.

Tests continue to isolate their schema/data within the disposable database. The local runner removes the whole database container after the command, including on test failure.

### Manual Neon migration compatibility

[Backend School Neon Compatibility](../.github/workflows/backend-school-neon-compatibility.yml) is an explicit `workflow_dispatch` gate. Configure names only in repository settings; never put their values in source:

```text
Secret:    NEON_TEST_API_KEY
Variables: NEON_TEST_PROJECT_ID
           NEON_TEST_PARENT_BRANCH_ID
           NEON_TEST_DATABASE
           NEON_TEST_ROLE
```

The project and parent branch must be dedicated to testing and contain no production data. Each confirmed run creates a unique ordinary copy-on-write child branch from that parent, passes the action's direct `db_url` to migration/schema tests, and deletes the exact created branch in an unconditional finalizer. The tests create isolated schemas and run the active migrations themselves, so the parent needs only the configured empty database and owner role. Compute suspension is set to the Free-plan-compatible 300 seconds; do not shorten it or omit it because the pinned action maps omission to disabled auto-suspend. The two-hour expiration is a fallback if finalization cannot run. The gate never uses the pooled endpoint because transaction pooling can expose the wrong schema-local `_sqlx_migrations` state.

The backend static architecture suite validates that active migrations remain a contiguous timeline beginning at `001_baseline.sql`. Runtime rollout and all-tenant migration verification are documented in [Operations](./OPERATIONS.md).

## Encryption and PII

For changes to encryption, national IDs, blind indexes, or admission PII, from `backend-school`:

```bash
cargo test utils::field_encryption::tests --bin backend-school
cargo test modules::admission::services::pii::tests --bin backend-school
cargo check
```

These focused tests use test-only keys. Never put a real national ID, `ENCRYPTION_KEY`, or `BLIND_INDEX_KEY` in source, fixtures, command output, screenshots, or logs.

## Smoke Tests

The repository smoke script checks frontend reachability, backend liveness/readiness, CORS and CSRF preflight, legacy-cookie rejection, opaque-session login, `/api/auth/me`, the active-session list, realtime SSE, optional private files, and current-session logout.

```bash
SMOKE_SUBDOMAIN=sandbox \
SMOKE_USERNAME=T0001 \
SMOKE_PASSWORD='provided-at-runtime' \
./scripts/smoke_test.sh
```

Alternatively, copy `.env.smoke.example` to the ignored `.env.smoke.local`. The script loads that file by default; `SMOKE_ENV_FILE` can point elsewhere. Credentials must come from `SMOKE_*` environment variables or the ignored environment file and must never be committed.

If credentials are absent, authenticated checks are skipped. Report that limitation rather than describing the smoke suite as fully passing.

The API-domain cookie is `__Host-schoolorbit_session`; it is opaque and unavailable to frontend JavaScript. Login and authenticated `/api/auth/me` responses expose `X-CSRF-Token`. The smoke script captures that value only in a shell variable, updates it after rotation-capable responses, and sends it on every authenticated `POST`, `PUT`, `PATCH`, or `DELETE`. Never print, export, persist, or enable trace output for the CSRF value. A manual flow should use private temporary files:

```bash
cookie_jar=$(mktemp)
headers_file=$(mktemp)
chmod 0600 "$cookie_jar" "$headers_file"
# Capture X-CSRF-Token from login or /api/auth/me into csrf_token without printing it.
# Remove both files and unset csrf_token when the check ends.
```

### File Platform smoke

Run this only against an isolated tenant with no retained files. The authenticated account must be allowed to update school settings for the public-logo case; the private-profile case operates on the logged-in user. Supply a small valid PNG through `FILE_SMOKE_PNG`; the repository smoke script creates and removes its own private cookie jar. Never commit the PNG, cookie jar, or captured CSRF value.

1. Upload `school_logo` through `POST /api/files`; retain only the returned file ID.
2. Confirm authenticated metadata from `GET /api/files/{id}` contains `publicContentUrl` but no bucket, object key, storage path, provider URL, or signed URL.
3. Confirm anonymous `GET /api/public/files/{id}/content` redirects and delivers the PNG. Also request `GET /api/public/files/{id}/delivery`, retain `data.url` only in memory, fetch it as a separate credential-free request with the tenant `Origin` and no referrer, and confirm a non-empty PNG plus matching `Access-Control-Allow-Origin`. Never print or persist the delivery URL.
4. Upload `profile_image` through `POST /api/files`; confirm anonymous metadata/download fails.
5. Confirm authenticated `POST /api/files/{id}/download` returns a `200` typed grant without bucket, object-key, or provider details. Keep `data.url` in memory, fetch it separately with the tenant `Origin` and credentials omitted, and confirm it writes bytes to a temporary output file. Never print the response body or grant URL because the URL is a temporary bearer credential.
6. Delete each file with `DELETE /api/files/{id}`. Repeat delete through the owning domain workflow where supported, and confirm delivery remains revoked even if object cleanup is pending.
7. Search backend and proxy logs for leaked signed-query markers, object-key prefixes, filenames, or content. A file ID and safe error code are allowed.

Representative requests, with credentials and IDs supplied only at runtime:

```bash
curl -fsS -b "$FILE_SMOKE_COOKIE_JAR" \
  -H "Origin: $SMOKE_ORIGIN" \
  -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
  -H "X-CSRF-Token: $csrf_token" \
  -F purpose=school_logo \
  -F "file=@$FILE_SMOKE_PNG;type=image/png" \
  "$SMOKE_API_URL/api/files"

curl -fsSL \
  -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
  "$SMOKE_API_URL/api/public/files/$PUBLIC_FILE_ID/content" \
  -o /dev/null

curl -fsS -b "$FILE_SMOKE_COOKIE_JAR" \
  -H "Origin: $SMOKE_ORIGIN" \
  -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
  -H "X-CSRF-Token: $csrf_token" \
  -F purpose=profile_image \
  -F "file=@$FILE_SMOKE_PNG;type=image/png" \
  "$SMOKE_API_URL/api/files"

curl -fsS -X DELETE -b "$FILE_SMOKE_COOKIE_JAR" \
  -H "Origin: $SMOKE_ORIGIN" \
  -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
  -H "X-CSRF-Token: $csrf_token" \
  "$SMOKE_API_URL/api/files/$FILE_ID" \
  -o /dev/null
```

Exercise private delivery through the typed frontend helper or an in-memory test harness that
parses the grant and provider response without writing or printing the URL. Do not pipe the
grant envelope through verbose shell output or enable HTTP tracing.

For scanner failure behavior in a non-production environment:

1. stop `schoolorbit-clamd`;
2. confirm `/health` remains `200` and `/ready` becomes `503`;
3. confirm a valid upload returns `503` and creates neither ready metadata nor an object;
4. restart clamd, wait for `clamdcheck.sh`, and confirm `/ready` returns `200`;
5. upload the standard EICAR anti-malware test file and confirm it is rejected without becoming ready or publicly deliverable.

Run the focused adapter tests as well:

```bash
cd backend-school
cargo test modules::files::runtime_config --bin backend-school -- --nocapture
cargo test modules::files::malware_scanner --bin backend-school -- --nocapture
cargo test modules::files::r2_storage_provider --bin backend-school -- --nocapture
cargo test modules::files::platform_service --bin backend-school -- --nocapture
cargo test modules::files::reconciler --bin backend-school -- --nocapture
```

## Browser E2E

From `frontend-school`:

```bash
E2E_BASE_URL='https://sandbox.schoolorbit.app' \
E2E_USERNAME='provided-at-runtime' \
E2E_PASSWORD='provided-at-runtime' \
npm run test:e2e
```

`SMOKE_TENANT_URL`, `SMOKE_SUBDOMAIN`, `SMOKE_USERNAME`, and `SMOKE_PASSWORD` are accepted fallbacks only for the non-destructive login spec. The backend sets `__Host-schoolorbit_session` for the API domain, so assertions inspect Playwright browser-context cookies rather than only cookies visible for the tenant page domain.

Run destructive multi-context session coverage separately with a dedicated disposable account:

```bash
E2E_BASE_URL='https://sandbox.schoolorbit.app' \
E2E_API_URL='https://school-api.schoolorbit.app' \
E2E_SESSION_USERNAME='dedicated-disposable-account' \
E2E_SESSION_PASSWORD='provided-at-runtime' \
npx playwright test tests/e2e/session-security.spec.ts
```

Set `E2E_OTHER_TENANT_URL` to another tenant when tenant-isolation proof is available; only that optional case skips when the variable is absent. The suite never changes the account password.

Use `npm run test:e2e:headed` only when interactive debugging is needed. Retain traces, screenshots, and videos only when they contain no sensitive data.

## Realtime Rollout Checks

When WebSocket identity, proxying, or authentication changes:

1. Verify the handshake without legacy query identity fields.
2. Search proxy/backend access logs for unexpected query parameters, while ensuring sensitive query strings are not logged.
3. Confirm no deployed client sends legacy query identity parameters: `user_id`, `name`, or `school_key`.
4. Confirm the backend derives actor and tenant identity from authenticated server context and fails closed when authorization is lost.
5. Confirm ping/pong heartbeat, stale-client cleanup, reconnect backoff, and permission-change disconnect/refresh behavior through the reverse proxy.

Do not log tokens, cookies, identity query values, or full request URIs during rollout checks.

## Ubuntu 26.04 Playwright

Until Playwright provides native Ubuntu 26.04 browser builds, install and run with:

```bash
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 npx playwright install chromium
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 npm run test:e2e
```

Do not rely on `npx playwright install-deps` on Ubuntu 26.04 when it requests unavailable Ubuntu 24.04 packages. Install required native packages explicitly when needed:

```bash
sudo apt install -y libnspr4 libnss3 libasound2t64 libxss1 fonts-liberation
```

The sandbox E2E workflow uses Ubuntu 24.04.
