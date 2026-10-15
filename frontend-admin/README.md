# Frontend Admin

## Purpose

The apex-domain public site and customer portal manage email-verified accounts,
school service requests and monthly/yearly service periods. Customers see only
schools explicitly assigned by an operator. Expiry warns without restricting
school access. Operators retain provisioning, deployment and migration tools;
school deletion is unavailable. The theme follows the public homepage.

## Stack

- SvelteKit 3 and Svelte 5
- TypeScript 6 and Vite 8 (Node.js 24)
- Tailwind CSS
- Cloudflare adapter

## Local Setup

```bash
cd frontend-admin
cp .env.example .env
npm ci
```

## Development

```bash
npm run dev
```

Use `npm run preview` to inspect a production build locally.

## Check and Build

```bash
npm run lint
npm run check
npm run build
```

## Environment

- `PUBLIC_API_URL` points browser requests to backend-admin.
- The Worker has no internal-service credentials. Migration operations use the
  authenticated backend-admin operator API, never a Worker-held internal secret.

For production deployment, `BACKEND_ADMIN_URL`, `BASE_DOMAIN`, and
`CLOUDFLARE_ACCOUNT_ID` are GitHub repository variables. The workflow maps
`BACKEND_ADMIN_URL` to build-time `PUBLIC_API_URL`, routes the apex to the Worker,
and retains `admin.<base-domain>` solely for a permanent redirect.

Public browser variables must use the `PUBLIC_` prefix. Never expose internal secrets as public variables. A committed Wrangler file defines only environment-neutral application configuration; it never owns production credentials or URLs. The deployment workflow generates its environment-specific Wrangler file at runtime.

## Project Documentation

- [Development rules](../.rules)
- [Testing](../docs/TESTING.md)
- [Operations](../docs/OPERATIONS.md)
