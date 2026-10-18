# Correspondence directed routing implementation

The product decisions and preservation guarantees are recorded in [the approved scope design](../specs/2026-10-18-correspondence-directed-routing-design.md). Senior owns the shared Git index, PR, integration and deployment. Backend owns workflow persistence, transport/contracts, the forward migration and generated API artifacts; Frontend owns correspondence screens and browser coverage. Review and QA independently inspect the frozen candidate.

1. Inspect current process/correspondence, migrations, organization/acting authority, Work, Notifications, API composition and frontend consumers. Agree the concrete typed wire contract before dependent runtime edits.
2. Add only forward settings/graph persistence and typed models, retaining legacy request receipt serialization and every old process/template/book/evidence path.
3. Implement explicit directed submission, recipient resolution, graph transitions, optional/inherited review and parent completion using existing row locks and transactional central work/notification operations.
4. Add thin authenticated settings/selection/action handlers and API registrations. Update read, upload, list counts, list progress, detail actions and work eligibility from the same graph authority. Generate contracts using the existing owner.
5. Implement first-decision-maker selection, routing/review/delegation controls and tracking in the Frontend with server capabilities and generated DTOs. Keep fixed templates and all book registers meaningful.
6. Run focused backend graph/authority/replay/legacy database coverage and affected frontend browser coverage, then default/documentation builds, permission/API generation checks, formatting and documentation policy checks. Inspect the complete diff and retain evidence for the tested inputs.
7. Freeze the integrated candidate for independent Reviewer and QA. Assign repairs to their owners, repeat affected acceptance, then let Senior deliver through the authorized existing PR/integration/deployment workflow.

Planned Backend commands, using the existing compatible shared Cargo target:

```bash
./scripts/test_backend_school.sh --package school-workflow --test correspondence -- --test-threads=2
cd backend-school
cargo check --locked -p school-workflow --all-targets
cargo check --locked -p backend-school --features api-docs
cargo fmt --all --check
cd ../frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
cd ..
./scripts/pipeline verify --scope documentation
git diff --check
```

Permission generation/checks are required if investigation requires an additional permission rather than the existing process/correspondence capabilities. No dependency/toolchain change is planned.
