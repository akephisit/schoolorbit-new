# Frontend Admin

## Purpose

The administration web application supports school provisioning and control-plane operations through backend-admin and selected server-side backend-school calls.

## Stack

- SvelteKit 5 and Svelte 5
- TypeScript and Vite
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
- `BACKEND_SCHOOL_URL` and `INTERNAL_API_SECRET` support server-side migration coordination.

For production deployment, `BACKEND_ADMIN_URL`, `BACKEND_SCHOOL_URL`, `BASE_DOMAIN`, and `CLOUDFLARE_ACCOUNT_ID` are GitHub repository variables. The workflow maps `BACKEND_ADMIN_URL` to the build-time `PUBLIC_API_URL`. `INTERNAL_API_SECRET` is a Cloudflare Worker secret binding supplied from the GitHub repository secret with the same name.

Public browser variables must use the `PUBLIC_` prefix. Never expose internal secrets as public variables. A committed Wrangler file defines only environment-neutral application configuration; it never owns production credentials or URLs. The deployment workflow generates its environment-specific Wrangler file at runtime.

## Project Documentation

- [Development rules](../.rules)
- [Testing](../docs/TESTING.md)
- [Operations](../docs/OPERATIONS.md)
