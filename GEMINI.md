# SchoolOrbit — project instructions

Project conventions are documented in [`.rules`](./.rules). Please read that file first — it covers:

- **Analysis workflow** before any change (consult `/docs/`, identify stack impact, plan, verify)
- **Backend (Rust + Axum + sqlx)** — error handling without panics, thin handlers, AppState patterns
- **Frontend (SvelteKit 5 + TypeScript + Tailwind)** — `+page.server.ts` loaders, Svelte stores, custom API client
- **Realtime / WebSocket** — heartbeat ping/pong, reconnection strategy, ConnectedClients in AppState
- **PDPA / Security** — `national_id` MUST be AES-GCM encrypted + HMAC blind-indexed, never logged plaintext
- **Migration safety** — never edit applied migrations; add a new migration even for DB comments
- **Deployment** — env-driven config, container networking, port binding to `0.0.0.0`
- **Testing** — sandbox smoke tests, Playwright E2E, Ubuntu 26.04 Playwright caveats

## Key references

- `/docs/` — architecture, feature guides, setup
- `/docs/TESTING.md` — local backend checks, migration safety, smoke tests, browser E2E
- `/IMPROVEMENT_PLAN.md` — outstanding refactor items (priority C/H/M/L)
- `/TODO_ENCRYPTION.md` — current encryption standard and operational notes
- `/backend-school/migrations/` — schema evolution
- `/backend-school/src/modules/` — feature-based module structure (handlers + services + models)

## Project conventions worth knowing

These are easy-to-miss patterns that have tripped up dev/AI in the past.

### Re-export pattern in `frontend-school/src/lib/components/ui/`
Sub-components (e.g. `calendar/calendar-day.svelte`) are imported via `index.ts` aliases, not by filename. Searching for `import.*calendar-day` will miss usage. To verify a component is used, grep the `index.ts` re-exports first, then grep for the aliased name (`Day`, `Cell`, `Grid`, ...).

### Encryption: AES-GCM app-side is the standard
Use `backend-school/src/utils/field_encryption.rs` for encrypted fields. `national_id` values are stored as app-side AES-256-GCM ciphertext using `ENCRYPTION_KEY`; `*_national_id_hash` values are keyed HMAC-SHA256 blind indexes using `BLIND_INDEX_KEY`. Legacy PostgreSQL `pgcrypto` helpers were removed; do not reintroduce that path.

### Migration files are immutable after apply
Do not edit old migration files, even to fix comments. `sqlx` checks migration checksums and tenant startup will fail if an applied file changes. Add a new sequential migration for schema, index, data, or comment changes.

### Timetable API split: `/api/me/` vs `/api/academic/`
- `GET /api/me/timetable` — self-view for student/staff (backend resolves user_id from JWT, no permission required)
- `GET /api/academic/timetable?...` — admin view (requires `ACADEMIC_COURSE_PLAN_READ_ALL`)
- `GET /api/parent/students/{id}/timetable` — parent view (verifies parent-child link)

All three resolve to the same `timetable_service::list_entries()` under the hood — different filters, single source of truth.

### Service layer pattern (in progress)
Business logic lives in `modules/<feature>/services/<feature>_service.rs`. Handlers should be thin: permission check → service call → format response (+ broadcast WS events). The `academic/timetable` module is the reference example — see `services/timetable_service.rs`. Other modules (admission, staff, study_plans) still have fat handlers and are pending refactor.

Pattern: services receive `&PgPool` and return `Result<T, AppError>` or an outcome enum (`CreateEntryOutcome`, `SwapOutcome`, etc.) so handlers can decide HTTP/WS responses without coupling business logic to Axum.

### Sandbox smoke/E2E testing
Use `scripts/smoke_test.sh` for API/CORS/auth smoke checks and `frontend-school` Playwright tests for browser login checks. Credentials must come from env vars (`SMOKE_*` or `E2E_*`), never committed. On Ubuntu 26.04 local machines, use `PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64`; see `.rules` and `docs/TESTING.md` for the native package caveat.

### Common analysis pitfalls in this repo

Sweeping analysis (especially via AI sub-agents) has produced false claims here. When you see a sweep report something as unused/broken/needs-migration, verify with direct grep/Read against the current state first.

- **"Components in `ui/calendar/` are unused"** — they re-export via `index.ts` aliases (`Day`, `Cell`, `Grid`), not by filename. Check the index re-exports before declaring dead code.
- **"Column X needs a sync trigger / migration"** — read the migration *timeline*, not just the creation migration. Several columns (e.g. `subjects.level_scope`) were dropped in later migrations and replaced with junction tables.
- **"`CLAUDE.md`/`AGENTS.md`/`GEMINI.md` are empty placeholders"** — these are auto-loaded entry points for AI tools; minimal content ≠ useless.
