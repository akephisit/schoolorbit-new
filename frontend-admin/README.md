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

Public browser variables must use the `PUBLIC_` prefix. Never expose internal secrets as public variables.

## Project Documentation

- [Development rules](../.rules)
- [Testing](../docs/TESTING.md)
- [Operations](../docs/OPERATIONS.md)
