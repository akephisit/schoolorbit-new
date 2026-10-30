# Timetable live collaboration implementation

Approved design: [live collaboration](../specs/2026-10-30-timetable-live-collaboration-design.md).
Senior owns Git and integration. Backend owns server/domain contracts and their generator;
Frontend owns browser state, board interaction and browser coverage. Reviewer and QA independently
accept the frozen integrated candidate. Shared checkout: `feat/timetable-live-collaboration`.

1. Backend defines domain DTOs and schema registration, then communicates the exact protocol to
   Frontend. Add pure memory reservation, ownership, TTL, journal and projection coverage.
2. Extract one canonical visibility projector shared by HTTP and runtime. Add the bounded Staff
   portrait port without changing Files authorization. Compose one runtime into HTTP and sockets.
3. Connect draft membership, semantic presence, claims, private target availability and replay to
   existing authenticated socket lifecycle and retained term-wide signals.
4. Require mutation context/permits across every true write owner. Keep accepted work alive through
   transaction and ordered cache/delta publication, including cancellation and lifecycle cleanup.
5. Frontend integrates generated DTOs, per-tab ownership, roster/context jumps, semantic anchors,
   sampled/interpolated cursors and ghosts, reservations and patch application.
6. Backend runs the sole API generator after schema coherence and notifies Frontend. Update only
   durable realtime ownership and verification/recovery documentation.
7. Senior freezes the candidate for independent review and QA, assigns findings to each owner,
   checkpoints the draft PR, and completes the authorized integration workflow.

Focused backend verification uses `cargo test -p school-academic-timetable` for pure runtime and
projection behavior, `./scripts/test_backend_school.sh --package school-academic-http
websockets::security_tests -- --nocapture`, and disposable timetable service/lifecycle/template/
handoff and HTTP tests through `scripts/test_backend_school.sh`. Run affected Staff portrait,
auth/session, static architecture and API schema checks. The sole generator runs
`npm run generate:api-contracts`, `npm run check:api-contracts` and `npm run test:api-contracts`
from `frontend-school`. Frontend runs its owning Svelte/type/lint tests and focused browser tests.
Existing session Playwright discovery and direct/proxy smoke remain required for the changed
realtime boundary. Finish with `git diff --check`, complete diff review and `git status --short`.

Runtime reservation/replay tests require no database. Database tests use only runner-owned
PostgreSQL fixtures; read the managed cloud runtime Docker instructions before creating them.
No production mutation is an acceptance test. No migration or durable reservation table exists.
