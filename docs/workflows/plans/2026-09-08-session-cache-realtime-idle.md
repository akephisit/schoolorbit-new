# Session cache and realtime idle implementation plan

Goal: reduce tenant database reads for one backend process while retaining expiry,
rotation, CSRF, tenant isolation, and immediate application-driven invalidation.

The user approved all four improvements and specified one backend instance.
Use bounded process-local caches: authentication for 60 seconds, realtime database
validation for 5 minutes; never extend idle/absolute/token-maintenance deadlines.
Permission-cache invalidation also invalidates identity caches. Session mutations
invalidate before returning; in-flight reads cannot repopulate invalidated state.
Keep transport heartbeat at 30 seconds. Pause hidden-tab realtime after 60 seconds
and reconcile authoritative state on return. No migrations or wire changes.

- [x] Backend: add query-count/security regression tests; implement bounded caches,
  coalescing, session mutation invalidation and identity mutation coverage.
- [x] Frontend: test and implement hidden-tab pause/resume for SSE and timetable
  WebSocket, generation-safe cleanup, missed-event reconciliation and auth refresh
  coalescing. Use existing runtime test harnesses.
- [x] Review expiry/rotation, revocation races, tenant isolation, reconnect lifecycle,
  errors, memory bounds, and any uncovered identity mutation call sites.
- [x] Update `.rules` and `docs/OPERATIONS.md` for single-process cache ownership and
  direct-database-change freshness limits; add test recipes to `docs/TESTING.md`.
- [x] Run focused Rust auth/realtime tests with `scripts/test_backend_school.sh`,
  cargo fmt/check/static_architecture, frontend lint/check/test:static, session
  Playwright discovery and deployed-proxy smoke. Report unavailable external gates.
  Auth: 82 passed using disposable PostgreSQL. Pure SSE/WebSocket: 6/20 passed.
  Architecture: 163 passed. Frontend static: 620 passed; lint/check passed.
  Playwright discovered four tests. Deployed-proxy smoke passed unauthenticated checks only.
- [x] Review final diff, `git diff --check`, and `git status --short`.
- [ ] Rollout-only gate: authenticated proxy smoke and actual browser execution
  require a dedicated disposable account (`SMOKE_USERNAME`, `SMOKE_PASSWORD`,
  `E2E_SESSION_USERNAME`, `E2E_SESSION_PASSWORD`) and a target running this change.
  Those credentials were absent; this branch has not been deployed.
