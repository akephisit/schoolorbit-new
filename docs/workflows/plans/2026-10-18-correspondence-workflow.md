# Correspondence and workflow implementation plan

1. Read current owners, migration timeline, Files and notification provider APIs; establish typed API contract with Frontend. Preserve existing workflow/window consumers.
2. Add migration 121, closed typed DTOs, versioned template seeds, permission contracts and focused validation/policy helpers. Checkpoint this meaningful persistence/API boundary under Coordinator Git ownership.
3. Implement transactional reusable process start/action/completion, recipient snapshots, direct work inbox projections, notification producer snapshots and revision/idempotency history.
4. Implement correspondence ACL/list/detail/create/revision/send/actions, private document/signature File Platform policy and transaction attachment, immutable signing evidence and server PDF export.
5. Compose thin authenticated routes/narrow state; register OpenAPI schemas/paths and regenerate permission/API artifacts. Frontend consumes generated DTOs through its assigned owner.
6. Run `cargo fmt --all -- --check`, focused `cargo test -p school-workflow`, disposable DB `./scripts/test_backend_school.sh --package school-workflow`, affected Files tests, default/api-docs checks, API and permissions generation/check/tests, architecture and documentation checks. Select workspace coverage where composition/shared contract impact requires it. Record exact commands and failures in chat.
7. Freeze under Coordinator; independent Reviewer and QA evaluate integrated candidate. Repair owner-assigned findings, regenerate affected contracts and repeat checks before final handoff.

Backend owns backend-school, permission sources/registries, API artifacts and this approved spec/plan. Frontend owns other frontend source/tests. Coordinator owns all Git mutations, checkpoint barriers, PR/integration and deployment. No writer may stage/commit, spawn additional agents or alter another writer's paths.
