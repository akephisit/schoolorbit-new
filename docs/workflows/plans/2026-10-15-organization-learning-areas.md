# Organization and learning-area implementation

Base: origin/main 1acb6c5ba158c628d5042cfc606ffa19a4498118.
See the approved design in ../specs/2026-10-15-organization-learning-areas-design.md.

1. Add forward migration 116: validate mappings/status/owners, preserve identities and grants,
   separate the hierarchy, backfill missing owners, add counters and transactional projection.
2. Remove caller-owned codes from create contracts and learning-area updates. Return the
   linked organization ID. Guard generic organization writes and learning-area status changes.
3. Add permission-aware route loading for the active tab only. Redirect the academic registry;
   reuse Staff member/permission components and keep selection/detail in the URL.
4. Replace code-entry forms with name-first drafts, optional English/contact information,
   automatic ordering, durable errors and duplicate-submit guards.
5. Run focused real PostgreSQL migration/transaction/concurrency tests, allowed/denied policies,
   contract export/check/tests, application type/lint checks and the affected mocked browser
   suites on desktop/mobile and light/dark themes. Review final diff and source equality.
6. Record evidence in the PR. Remove these workflow inputs once its description records the
   completed outcome. Keep incomplete verification in draft; follow the normal Pipeline gate
   and operations acceptance for integration/release.
