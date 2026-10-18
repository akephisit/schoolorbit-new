# Staff leave implementation

Owners: Backend writer owns Staff/Workflow providers, migration, permission/API contracts and generated registries; Frontend writer owns non-generated frontend; Senior owns Git; independent Reviewer and QA assess the frozen candidate.

1. Define typed models and immutable category profiles; confirm exact shared API with Frontend.
2. Add migration 126 for types, policies, revisions, receipts, requests, audit and private file links; seed editable example types and permission/menu projection.
3. Extend Workflow public signature validation and source-authorized withdrawal, retaining provider ownership of work/notifications.
4. Implement Staff policy resolution, validated server-derived periods, union access, transactional reservations and replay-safe lifecycle/cancellation.
5. Wire thin protected HTTP handlers/OpenAPI, generate API and permission artifacts, and implement frontend workflows against that contract.
6. Verify focused pure and disposable Postgres cases for allowed/denied scopes, policy selection, quotas, stale/replay/rollback, signature consent and cancellation; compile affected providers/consumers; run contract generation/check/tests and documentation policy.
7. Freeze changes for independent review and QA, repair assigned findings, then hand off evidence and remaining limitations to Senior. No production, main, readiness or independent Git mutation.

Checks: `cargo fmt --all -- --check`; focused `cargo check -p school-staff -p school-workflow`; `./scripts/test_backend_school.sh --package school-staff leave -- --nocapture`; focused Workflow tests; `cargo check --workspace --all-targets` for shared composition; frontend `generate/check/test:permissions` and `generate/check/test:api-contracts`; documentation policy and `git diff --check`. Disposable Docker 18 fixtures are runner-owned. Reuse the existing pinned Rust installation and compiled Cargo target, serialize Cargo with the team.
