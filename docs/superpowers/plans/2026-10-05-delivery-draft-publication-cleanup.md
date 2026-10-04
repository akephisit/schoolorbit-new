# Delivery draft publication and cleanup implementation

1. Add forward schema changes for nullable draft dates/reference dates and DELETE-only cancelled guards. Preserve published data and test 088→new migration.
2. Add typed semantic changes, candidate-date preview/publication, delivery hard deletion and cancelled timetable deletion with scoped HTTP contracts. Retire cancellation APIs; generate contracts.
3. Update visible change region, name/diff rendering, preliminary readiness, publication date dialog, hard-delete confirmation and deletion reconciliation.
4. Test repeated findings, all semantic diff actions, dated teacher capture, publication concurrency/hash/idempotency, deletion references/rollback/scopes and cancelled cleanup atomicity.
5. Run frontend lint/check/static/menu-sync/route-loading and API contract checks; Rust focused/migration/architecture tests and workspace all-targets check; Playwright mobile/desktop light/dark/keyboard; git diff --check.
6. Rehearse active-tenant copies, retain recovery, squash-integrate exact verified tree, push and coordinated deploy. Smoke/accept before exact-scope cancelled cleanup; verify disappearance, published fingerprints and audit afterward.
