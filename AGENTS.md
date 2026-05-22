# SchoolOrbit — project instructions

Project conventions are documented in [`.rules`](./.rules). Please read that file first — it covers:

- **Analysis workflow** before any change (consult `/docs/`, identify stack impact, plan, verify)
- **Backend (Rust + Axum + sqlx)** — error handling without panics, thin handlers, AppState patterns
- **Frontend (SvelteKit 5 + TypeScript + Tailwind)** — `+page.server.ts` loaders, Svelte stores, custom API client
- **Realtime / WebSocket** — heartbeat ping/pong, reconnection strategy, ConnectedClients in AppState
- **PDPA / Security** — `national_id` MUST be encrypted + blind-indexed, never logged plaintext
- **Deployment** — env-driven config, container networking, port binding to `0.0.0.0`

## Key references

- `/docs/` — architecture, feature guides, setup
- `/IMPROVEMENT_PLAN.md` — outstanding refactor items (priority C/H/M/L)
- `/TODO_ENCRYPTION.md` — pending encryption work in staff/student handlers (C-3)
- `/backend-school/migrations/` — schema evolution
- `/backend-school/src/modules/` — feature-based module structure (handlers + services + models)
