# Timetable latest opening source implementation

Approved scope: [design](../specs/2026-11-01-timetable-latest-delivery-source-design.md). Base: `115e08882fc3a736a0cc92b1a23856b2a7b873db`; one shared feature checkout. Senior owns Git and checkpoints; Backend owns Rust, generated API artifacts and canonical documentation; Frontend owns other frontend paths. Independent Reviewer and QA own final acceptance.

## Implementation sequence

1. Save this settled design/plan as the first meaningful draft checkpoint. Pause all writer mutations until Senior confirms the checkpoint.
2. Change the closed source-alignment request to `rowVersion` only, add the narrow AlignDeliverySource policy and typed stale-source findings, and retire the dated applicable preview context. Reuse existing routes and generated contracts.
3. Implement one term-locked latest-source alignment owner. Preserve child rows and origin links; distinguish confirmed no-op from changed lifecycle outcomes without bypassing gate, authority, leases or uncertain-commit invalidation.
4. Make new initial/clone drafts use latest; dispatch existing initial/resume targets through Existing CommitPermit. Preserve term-preparation target mapping and published historical reads. Add the authoritative latest guard to content, template, handoff, placement-preview and publication boundaries; accepted receipts retain their earlier replay order.
5. Reconcile Timetable workspace lower bounds and backend/frontend state flow. Generate OpenAPI/TypeScript once Rust DTOs are stable and notify Frontend. Frontend replaces source selection with bounded authorized automatic alignment and fresh workspace/epoch recovery.
   Extend the existing generic error/HTTP owners with producer-coded conflicts; Timetable's generated one-value conflict data certifies only definitive pre-write outdated-source refusal. Keep ordinary errors unclassified. Preserve committed success if cached revision/delta publication fails. Retain failed captured intent for explicit review and a fresh Save; never retry automatically or clear all failures after GET. Include the promotion owner's mandatory exhaustive error-message match without changing its behavior.
6. Add focused public-service and real HTTP/collaboration regressions. Preserve previous assertions except the explicitly changed draft pin policy; explain each changed test requirement. Update `.rules`, `docs/TESTING.md` and relevant recovery guidance in `docs/OPERATIONS.md`.
7. Declare source complete, pause all source/generator/formatter writes, and hand off exact inputs, actual results and remaining checks to independent Review/QA. Repair assigned findings through Senior. Remove these finished workflow inputs only after the PR records the accepted outcome.

## Verification selection

Use the existing single Cargo target and pinned tools; serialize all heavy work. Every Cargo/generator invocation uses `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2` with the configured Cargo/Rustup/target paths. Read the cloud runtime and disposable database instructions before fixtures. Do not install dependencies, clean shared caches or edit applied migrations.

Focused backend commands, selected as their affected inputs stabilize:

```bash
./scripts/test_backend_school.sh integration_tests::academic::services::timetable_version_service_tests -- --test-threads=1
./scripts/test_backend_school.sh integration_tests::academic::services::timetable_block_service_tests -- --test-threads=1
./scripts/test_backend_school.sh integration_tests::academic::services::timetable_collaboration_tests -- --test-threads=1
./scripts/test_backend_school.sh integration_tests::academic::delivery::services_tests::publication_revision_tests -- --test-threads=1
./scripts/test_backend_school.sh --integration timetable_clone_runtime_permissions -- --test-threads=1
./scripts/test_backend_school.sh --integration delivery_versions -- --test-threads=1
cargo test --manifest-path backend-school/Cargo.toml -p school-academic-timetable --features api-docs collaboration -- --nocapture
./scripts/pipeline verify --scope documentation
```

Use the canonical API generation/check recipe and affected API-contract tests, architecture/database-execution guards, scoped formatting and diff checks. Frontend selects source-alignment, publication-date and native collaboration regressions from `docs/TESTING.md`. Do not repeat a broader suite solely to save a checkpoint; independent QA selects remaining evidence on the frozen complete candidate.

The shared AppError extension additionally requires default and `api-docs` workspace compilation and focused `school-errors`/`school-http` tests. Verify exact optional coded409 data versus an ordinary uncoded conflict, real stale HTTP saves with unchanged business rows, and postcommit cache failure reconciliation without changing accepted success. Use the existing compact owner tests for cancellation, leases and accepted publication/handoff replay.

Regression fixtures must exercise actual public operations and the shared HTTP/socket runtime. Include Manager-only and Publisher-only success, scoped/read-only denial, revoked queued authority, malformed older-source input, full child-graph preservation, no-op epoch stability, conflicting leases and cancellation, no-member upstream invalidation, stale content/publication refusal, immutable accepted replay, future source dates, consumed history and unrelated terms. Test production behavior rather than mirrored selector implementations.

No runtime, SQL, generation or verification result is claimed by this initial plan.
