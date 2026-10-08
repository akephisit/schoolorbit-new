# Frontend School

## Purpose

The tenant-facing web application provides staff, student, and parent workflows against backend-school.

Its homepage is an anonymous school website using existing branding, current-year student and homeroom aggregates, staff totals, and active organization units with all current members. The loader starts all three public reads concurrently, awaits school identity for at most three seconds to render the school name and SEO in the initial HTML, and streams statistics and organization separately. Each read can retry independently. School management continues through the existing login and authenticated routes; no separate content store or manually entered public totals are required.

Homepage indexing, canonical URLs, robots and sitemap share the policy in `src/lib/school-public/seo.ts`. The production tenant domain comes from the configured `PUBLIC_BACKEND_URL` hostname (`school-api.<base-domain>`). Sandbox, local, preview and reserved hosts are not indexable. Only production school homepages enter sitemaps; other HTML responses use `noindex`. Search Console ownership and submissions are described in [Operations](../docs/OPERATIONS.md).

SEO logo URLs use `/school-logo` on the school's own origin. This endpoint resolves only the current school crest through the existing anonymous branding and public file-delivery APIs, then redirects to its public image. It forwards the tenant Origin without session cookies; callers cannot supply a file ID.

Spreadsheet import accepts `.xlsx` and UTF-8 `.csv`. Convert legacy `.xls` files before importing student IDs. ExcelJS owns Excel file reading and writing; SSF preserves displayed cell values such as zero-padded IDs during import.

## Stack

- SvelteKit 3 and Svelte 5
- TypeScript 6 and Vite 8 (Node.js 24)
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
- `npm run sync:menu-routes` requires server-only `DEPLOY_KEY` and `SUBDOMAIN`; Pipeline
  synchronizes routes through VPS loopback before acceptance and fails if synchronization is incomplete.

Do not expose backend secrets through `PUBLIC_*` or Vite variables.

## Project Documentation

- [Development rules](../.rules)
- [Testing](../docs/TESTING.md)
- [Operations](../docs/OPERATIONS.md)
