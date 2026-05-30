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
