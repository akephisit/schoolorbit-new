# Implementation plan

1. Refresh main and trace current release, migration, smoke, replay, installer and provisioning contracts.
2. Implement a tested change planner and shared local/CI verification runner. Consolidate CI gates and cache owners. Preserve real required suites, isolated database fixtures and pinned toolchains.
3. Add pre-maintenance artifact preparation and a globally serialized release controller; reuse existing verified migration, origin, Worker recovery and browser acceptance implementations. Make reusable workflows callable only.
4. Add universal maintenance and immutable accepted-state transitions for all four components, including failure retention, stale-candidate refusal and exact request correlation. Update installer/provisioning callers.
5. Add serialized collaborator PR auto-merge without required approval and document repository settings, local commands and team rules.
6. Run focused Node tests, frontend static/lint/typecheck, shellcheck, shfmt, Bats, production Compose resolution and Docker actionlint; run backend checks for changed Rust callers and disposable PostgreSQL tests. Inspect diff and exact-tree provenance.
7. Open a PR, attach it, verify CI on the final candidate and refresh main before integration. Check deploy, maintenance, smoke/browser acceptance and timing evidence. Remove these workflow input artifacts once the PR records the completed result.
