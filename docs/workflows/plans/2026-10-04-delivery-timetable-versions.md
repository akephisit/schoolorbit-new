# Delivery/timetable version implementation

Design: `../specs/2026-10-04-delivery-timetable-versions-design.md`.

1. Inspect the full academic migration timeline, operational revision publication,
   teacher handoff receipts, timetable mutations/readers and route contracts.
2. Add typed delivery snapshots and their canonical version owner. Add migration
   expansion/backfill with identity/relationship reconciliation and database tests.
3. Move operational revision creation/preview/publication/cancellation to delivery
   versions. Remove implicit timetable cloning, target insertion and group sync.
4. Add standalone timetable source reconciliation, semantic comparison, guarded
   readiness/publication and atomic deletion; preserve resource authorization.
5. Switch timetable workspace, daily teaching, term preparation and handoff reads
   to pinned delivery sources. Remove the obsolete joint lifecycle after mapping.
6. Register/generate API contracts; implement delivery version selection and the
   read-first timetable editor, source issue presentation, publish/delete dialogs.
7. Execute focused tests while working; finish all frontend/backend/contract matrix
   checks, browser scenarios and sanitized visual inspection. Review final diff.
8. Fetch/integrate latest main on the feature branch, rerun affected checks, squash
   and push only after passing. Verify coordinated deployment, all-tenant migrations
   and read-only acceptance; retain branch until acceptance is complete.

Verification commands use `docs/TESTING.md`: `scripts/test_backend_school.sh` for
isolated database cases; `cargo test -p school-academic-delivery`,
`cargo test -p school-academic-timetable`, `cargo test --test static_architecture`,
`cargo fmt --all -- --check`, `cargo check --workspace --all-targets`;
frontend `generate:api-contracts`, `check:api-contracts`, `test:api-contracts`,
`check:permissions`, `test:permissions`, `lint`, `check`, `test:static`,
`test:menu-sync`, `test:route-loading`, build and relevant Playwright execution;
`git diff --check`, `git status --short`, release migration audits and smoke.
