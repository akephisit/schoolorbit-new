# Testing

This project uses a sandbox tenant for production-like smoke tests without touching real school data.

## Before Commit

Always run a diff sanity check from the repository root:

```bash
git diff --check
git status --short
```

Use focused tests for the code you changed. Do not run broad formatting or cleanup commands unless the task is specifically about formatting.

## Backend Checks

For `backend-school` changes:

```bash
cd backend-school
cargo check
```

For backend architecture guard changes (module roots, service-layer handler boundaries,
permission guards, tenant resolver rules, internal auth rules):

```bash
cd backend-school
cargo test --test static_architecture
```

For encryption or `national_id` / admission PII changes, also run:

```bash
cd backend-school
cargo test utils::field_encryption::tests --bin backend-school
cargo test modules::admission::services::pii::tests --bin backend-school
```

These tests require no real secrets; they set test-only `ENCRYPTION_KEY` and `BLIND_INDEX_KEY` internally.

For `backend-admin` changes:

```bash
cd backend-admin
cargo check
```

Existing backend warnings are tracked separately. Do not run `cargo fix` as part of unrelated work.

## Migration Safety

Never edit a migration that may already have been applied, even for comments. `sqlx` stores migration checksums and tenant startup will fail if an applied migration file changes.

Correct workflow:

1. Add a new sequential migration file.
2. Put schema, index, data, or comment changes in the new migration.
3. Run `git diff --check`.
4. Let backend-school apply the new migration during tenant startup or run the migration flow explicitly in a controlled environment.

If a checksum mismatch appears, restore the original migration file content. Do not edit the database migration checksum to hide the mismatch.

## Sandbox Smoke Test

Run the smoke test from the repository root:

```bash
SMOKE_SUBDOMAIN=sandbox \
SMOKE_USERNAME=T0001 \
SMOKE_PASSWORD='your-sandbox-password' \
./scripts/smoke_test.sh
```

Or copy the local env template and run the script without inline secrets:

```bash
cp .env.smoke.example .env.smoke.local
# edit .env.smoke.local and set SMOKE_PASSWORD
./scripts/smoke_test.sh
```

`scripts/smoke_test.sh` automatically loads `.env.smoke.local` by default. Override with
`SMOKE_ENV_FILE=/path/to/file` when needed.

The script checks:

- tenant frontend page loads
- backend-admin `/health`
- backend-school `/health`
- CORS from the tenant origin
- unauthenticated `/api/auth/me` returns `401`
- login preflight returns `204`
- login returns an `auth_token` cookie
- authenticated `/api/auth/me` returns the logged-in user

Production tenant browser requests use `Origin` for tenant resolution by default. Smoke tenant API requests still send both `Origin` and `X-School-Subdomain`; the preflight check includes `x-school-subdomain` so CORS config drift for explicit override clients is caught before browser E2E.

If `SMOKE_USERNAME` or `SMOKE_PASSWORD` is omitted, authenticated login checks are skipped and the script only validates public endpoints, CORS, and login request validation.

## Environment Variables

Optional overrides:

```bash
SMOKE_SUBDOMAIN=sandbox
SMOKE_TENANT_URL=https://sandbox.schoolorbit.app
SMOKE_ORIGIN=https://sandbox.schoolorbit.app
SMOKE_API_URL=https://school-api.schoolorbit.app
SMOKE_ADMIN_API_URL=https://admin-api.schoolorbit.app
SMOKE_TIMEOUT_SECONDS=20
SMOKE_REMEMBER_ME=true
```

Do not commit sandbox passwords or production credentials. Pass them as environment variables only.

## GitHub Actions

The `Smoke Test Sandbox` workflow can be run manually from GitHub Actions. It uses the same `scripts/smoke_test.sh` script and defaults to `sandbox.schoolorbit.app`.

For authenticated checks, configure repository secrets:

```bash
SMOKE_USERNAME
SMOKE_PASSWORD
```

Run it from Actions with `run_authenticated=true` to test login and authenticated `/api/auth/me`. Use `run_authenticated=false` for public endpoint and CORS checks only.

## Browser E2E

The `frontend-school` app has a minimal Playwright test for the live sandbox login flow.

Install the browser once on a local machine:

```bash
cd frontend-school
npx playwright install chromium
```

On Ubuntu 26.04, Playwright may need the Ubuntu 24.04 browser fallback until official 26.04 browser builds are available:

```bash
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 npx playwright install chromium
```

If Chromium launches with missing shared libraries such as `libnspr4.so`, install the native dependencies:

```bash
sudo apt install -y libnspr4 libnss3 libasound2t64 libxss1 fonts-liberation
```

Then verify the installed Playwright browser has no missing shared libraries:

```bash
ldd ~/.cache/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-linux64/chrome-headless-shell | grep "not found" || echo "deps ok"
```

Run the test:

```bash
E2E_BASE_URL=https://sandbox.schoolorbit.app \
E2E_USERNAME=T0001 \
E2E_PASSWORD='your-sandbox-password' \
npm run test:e2e
```

On Ubuntu 26.04, include the same platform override when running the test:

```bash
E2E_BASE_URL=https://sandbox.schoolorbit.app \
E2E_USERNAME=T0001 \
E2E_PASSWORD='your-sandbox-password' \
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
npm run test:e2e
```

The test also accepts `SMOKE_USERNAME` and `SMOKE_PASSWORD`, so the same secrets can be reused in GitHub Actions.

The `E2E Sandbox` workflow runs the same Playwright test manually from GitHub Actions. It expects repository secrets named `SMOKE_USERNAME` and `SMOKE_PASSWORD`, and is pinned to `ubuntu-24.04` for Playwright browser support.

## Sandbox Seed

Use the seed script when the sandbox tenant needs deterministic test data. The script is idempotent and only manages the sandbox fixtures it owns:

- staff/admin login user: `T0001` by default, or `SANDBOX_ADMIN_USERNAME`
- student login user: `SBX0001`
- parent login user: `P0001`
- active academic year and two semesters
- one `ม.1/1` classroom with the seeded student enrolled
- minimal study plan/version required by classroom creation

The script refuses to run against a non-`sandbox` subdomain or a database URL that does not look like sandbox unless `SANDBOX_ALLOW_NON_SANDBOX=1` is set.

Resolve the tenant database URL through backend-admin:

```bash
SANDBOX_SUBDOMAIN=sandbox \
SANDBOX_ADMIN_API_URL=https://admin-api.schoolorbit.app \
INTERNAL_API_SECRET='your-internal-secret' \
SANDBOX_SEED_PASSWORD='your-sandbox-password' \
./scripts/seed_sandbox.sh
```

Or pass the tenant database URL directly:

```bash
SANDBOX_DATABASE_URL='postgresql://...' \
SANDBOX_SEED_PASSWORD='your-sandbox-password' \
./scripts/seed_sandbox.sh
```

Optional overrides:

```bash
SANDBOX_ADMIN_USERNAME=T0001
SANDBOX_ACADEMIC_YEAR=2569
SANDBOX_STUDENT_PASSWORD='student-password'
SANDBOX_PARENT_PASSWORD='parent-password'
SANDBOX_SKIP_MIGRATIONS=1
```
