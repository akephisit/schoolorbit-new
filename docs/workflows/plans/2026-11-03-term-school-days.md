# Term school days implementation

Implements the approved [design](../specs/2026-11-03-term-school-days-design.md).

1. Add the fail-closed migration and shared Core weekday provider. Rename annual
   contracts to the initial `defaultSchoolDays` preset; add canonical term days.
2. Add revision-checked atomic copy-create/copy-into-target Core operations and
   HTTP paths, preserving independent schedule/period identities and all references.
3. Reconcile bell validation, activation diagnostics, Timetable eligibility and
   publication, and Attendance initial defaults through the Core provider.
4. Update Rust fixtures and contract assertions for the approved ownership change;
   regenerate OpenAPI and TypeScript from the Rust owner. Frontend consumes those
   contracts and implements previous-term copy defaults and term weekday editing.
5. Check migration preservation/refusal, source and target CAS, atomic rollback,
   historical/reference protection, NULL wildcard overlap, timetable denied days,
   Attendance initial and saved settings, and authenticated transport permissions.

Selected verification, serialized by the Backend then QA owner:

```bash
cargo fmt --manifest-path backend-school/Cargo.toml --all --check
cargo check --manifest-path backend-school/Cargo.toml --workspace --locked
cargo check --manifest-path backend-school/Cargo.toml --workspace --locked --features api-docs
./scripts/test_backend_school.sh --package school-academic-core -- --test-threads=2
./scripts/test_backend_school.sh term_school -- --test-threads=2
./scripts/test_backend_school.sh activation_context -- --test-threads=2
./scripts/test_backend_school.sh period_setup -- --test-threads=2
./scripts/test_backend_school.sh timetable_block_service_tests -- --test-threads=2
./scripts/test_backend_school.sh publication_source_tests -- --test-threads=2
cd frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

Run the focused Attendance, lifecycle and router cases that cover changed behavior,
and the owning frontend type/static/browser checks. Independent Reviewer and QA
assess the frozen candidate. Record real results in chat/PR rather than this plan.
