# Supervision consecutive-period implementation plan

1. Agree stable typed HTTP DTOs with the frontend owner. Add strict schedule-only booking requests, weekly options, ordered period responses and revision-checked cycle-deletion models.
2. Add forward migrations 164/165 with exact legacy evidence backfill, bijective full-graph reconciliation and cleanup refusal gates. Preserve old timestamps and raw unknown JSON without inferring ends.
3. Replace the single-lesson runtime with provider-composed weekly options, contiguous exact-homeroom selection and atomic canonical periods. Update all evaluator overlaps/locks, reporting, lifecycle revisions and source detachment.
4. Add permission-gated cycle deletion preview and atomic hard delete using the complete graph revision and canonical lifecycle lock order. Preserve shared resources and reject malformed graph references.
5. Add domain/database/transport/migration regressions; coordinate the canonical API generator with the Senior, then freeze source for independent Review and QA.

Backend owns school-supervision, new migrations, affected backend integration consumers, api_contract and canonical generated output; frontend source is owned separately. Senior owns Git/index/checkpoints/PR/integration. No production mutations or Git mutations by writers.

Selected checks: ./scripts/test_backend_school.sh supervision -- --test-threads=1; affected current academic/deletion/lifecycle/preparation and migration suites; cargo check -p school-supervision and affected root consumers in default/api-docs modes; scoped rustfmt and production Clippy panic guards; frontend-school npm run generate:api-contracts, check:api-contracts and test:api-contracts; documentation policy and git diff --check. All DB fixtures are disposable. Record exact executed checks and failures in the PR/task state, not another report. Retire this plan when the implementation PR records completion.
