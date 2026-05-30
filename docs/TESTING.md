# Testing

This project uses a sandbox tenant for production-like smoke tests without touching real school data.

## Sandbox Smoke Test

Run the smoke test from the repository root:

```bash
SMOKE_SUBDOMAIN=sandbox \
SMOKE_USERNAME=T0001 \
SMOKE_PASSWORD='your-sandbox-password' \
./scripts/smoke_test.sh
```

The script checks:

- tenant frontend page loads
- backend-admin `/health`
- backend-school `/health`
- CORS from the tenant origin
- unauthenticated `/api/auth/me` returns `401`
- login preflight returns `204`
- login returns an `auth_token` cookie
- authenticated `/api/auth/me` returns the logged-in user

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
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 npx playwright install-deps chromium
```

Run the test:

```bash
E2E_BASE_URL=https://sandbox.schoolorbit.app \
E2E_USERNAME=T0001 \
E2E_PASSWORD='your-sandbox-password' \
npm run test:e2e
```

The test also accepts `SMOKE_USERNAME` and `SMOKE_PASSWORD`, so the same secrets can be reused in GitHub Actions.

The `E2E Sandbox` workflow runs the same Playwright test manually from GitHub Actions. It expects repository secrets named `SMOKE_USERNAME` and `SMOKE_PASSWORD`, and is pinned to `ubuntu-24.04` for Playwright browser support.
