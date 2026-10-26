# Catalog grade snapshots implementation

1. Implement transaction-local grade normalization in the Academic Core catalog provider, preserving explicit nonempty coverage and checking tenant existence. Apply before duplicate comparison and persistence across course/activity create, update and publish.
2. Add focused service fixtures for all-grades snapshots, later grade additions, explicit partial/inactive scopes, invalid IDs, stale requests, no active grades and legacy draft publication. Save an early coordinated checkpoint.
3. Implement atomic migration 141 with deterministic corrective revisions, draft repair, archived exclusions, provenance mapping, refusal conditions, preservation and selection reconciliation, bounded audit evidence and post-cutover guards.
4. Preserve exact existing empty historical curriculum tuples on save. Keep program-copy behavior and all catalog pins intact; reject newly requested unsupported tuples.
5. Rehearse predecessor 140 to 141 on disposable PostgreSQL 18.6, including current/future/historical boundaries, same-day explicit winners, archived rows, no-op rerun and refusal/rollback/retry cases. Verify existing revision and curriculum selection regressions.
6. Document rollout and roll-forward recovery in Operations, coordinate Testing recipes with its frontend owner, and complete formatter/architecture/documentation checks selected under `.rules`.
7. Freeze the integrated candidate for independent review and QA; repair findings through assigned owners before Senior delivery.

Planned commands, once the Senior grants the shared Cargo window: `scripts/test_backend_school.sh catalog_grade_snapshot -- --test-threads=2`, focused `subject_revision`, `catalog_year_independence`, `curriculum_current_subject` and changed curriculum structure/copy cases; formatter and documentation-policy checks. Tests use the owned disposable database runner and shared target. API generation is needed only if investigation changes the unchanged DTO boundary.
