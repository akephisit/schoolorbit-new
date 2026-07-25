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

Database-backed tests use `TEST_DATABASE_URL`, never `DATABASE_URL`:

```bash
cd backend-school
TEST_DATABASE_URL='postgresql://.../schoolorbit_test' cargo test <focused-test>
```

Tests must isolate their schema/data and clean up what they create. With Neon, use a direct database endpoint for schema/search-path migrations. Do not use a `-pooler` transaction endpoint for these tests because shared session state can expose the wrong `_sqlx_migrations` table.

Read-only baseline validation:

```bash
./scripts/check_migration_rebaseline_ready.sh

MIGRATION_AUDIT_DATABASE_URL='postgresql://...' \
  ./scripts/check_migration_rebaseline_ready.sh
```

Guarded preparation/cutover procedures are documented in [Operations](./OPERATIONS.md).

## Encryption and PII

For changes to encryption, national IDs, blind indexes, or admission PII, from `backend-school`:

```bash
cargo test utils::field_encryption::tests --bin backend-school
cargo test modules::admission::services::pii::tests --bin backend-school
cargo check
```

These focused tests use test-only keys. Never put a real national ID, `ENCRYPTION_KEY`, or `BLIND_INDEX_KEY` in source, fixtures, command output, screenshots, or logs.

## Smoke Tests

The repository smoke script checks frontend reachability, backend liveness/readiness, CORS, login preflight, authentication, and `/api/auth/me`.

```bash
SMOKE_SUBDOMAIN=sandbox \
SMOKE_USERNAME=T0001 \
SMOKE_PASSWORD='provided-at-runtime' \
./scripts/smoke_test.sh
```

Alternatively, copy `.env.smoke.example` to the ignored `.env.smoke.local`. The script loads that file by default; `SMOKE_ENV_FILE` can point elsewhere. Credentials must come from `SMOKE_*` environment variables or the ignored environment file and must never be committed.

If credentials are absent, authenticated checks are skipped. Report that limitation rather than describing the smoke suite as fully passing.

## Browser E2E

From `frontend-school`:

```bash
E2E_BASE_URL='https://sandbox.schoolorbit.app' \
E2E_USERNAME='provided-at-runtime' \
E2E_PASSWORD='provided-at-runtime' \
npm run test:e2e
```

`SMOKE_TENANT_URL`, `SMOKE_SUBDOMAIN`, `SMOKE_USERNAME`, and `SMOKE_PASSWORD` are accepted fallbacks. The backend sets `auth_token` for the API domain, so assertions must inspect Playwright browser-context cookies rather than only cookies visible for the tenant page domain.

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
