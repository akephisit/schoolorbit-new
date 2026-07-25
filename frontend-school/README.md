# Frontend School

## Purpose

The tenant-facing web application provides staff, student, and parent workflows against backend-school.

## Stack

- SvelteKit 5 and Svelte 5
- TypeScript and Vite
- Tailwind CSS and local shadcn-svelte components
- Cloudflare adapter
- Playwright for browser E2E

## Local Setup

```bash
cd frontend-school
cp .env.example .env
npm ci
```

Set `PUBLIC_BACKEND_URL` to the backend-school URL. For localhost or a custom hostname, set `PUBLIC_SCHOOL_SUBDOMAIN` when automatic tenant detection is not possible.

## Development

```bash
npm run dev
```

Use `npm run preview` to inspect a production build locally.

## Check and Build

```bash
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
npm run build
```

API and permission types are generated contracts. Follow the workflows in [`.rules`](../.rules) instead of editing generated files.

## Environment

- `PUBLIC_BACKEND_URL` selects backend-school.
- `PUBLIC_SCHOOL_SUBDOMAIN` is an optional explicit tenant override.
- `PUBLIC_VAPID_KEY` configures Web Push.
- production menu registration requires both `VITE_DEPLOY_KEY` and `SUBDOMAIN`.

Do not expose backend secrets through `PUBLIC_*` or Vite variables.

## Project Documentation

- [Development rules](../.rules)
- [Testing](../docs/TESTING.md)
- [Operations](../docs/OPERATIONS.md)
