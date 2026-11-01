# Delivery publication revisions implementation plan

Implement the [approved design](../specs/2026-11-01-delivery-publication-revisions-design.md)
on the shared feature checkout. The Senior owns Git, migration-number reservation, checkpoints,
build scheduling and integration. Backend owns domain/migration/API/generated changes and canonical
verification/operations documentation. Frontend owns the remaining School UI and browser coverage.
The Data specialist supplies bounded read-only migration/consumer advice. Independent Reviewer
and QA assess the frozen complete candidate after implementation; writers repair assigned findings.

## 1. Reviewable design checkpoint

Record the settled sequence, immutable base graph, append-only effectivity and historical pin
semantics in the approved artifact. Pause all writing for the Senior's early draft PR checkpoint.
Resume runtime implementation only after that checkpoint is inspected and the Senior hands over
the shared build/generator slots. No further user approval is needed for the approved scope.

## 2. Canonical migration and domain projection

- Reserve migration 151 if still next on fresh main. Trace the full applied ownership timeline,
  especially 052/054/055/086–089/096/142–144/149/150; preserve all applied SQL bytes.
- Add independent Delivery/timetable publication numbers, backfill using original date/UUID order,
  validate lineage/shape/context and replace same-day uniqueness with term/number uniqueness.
- Add append-only Delivery teacher supersession and offering activation/cutoff evidence. Define
  canonical effective projections, immutable/context guards and provenance-aware teacher overlap
  validation. Retain original dates/roles/IDs and advisory serialization.
- Update snapshot projection to inherit immutable base teachers, apply explicit draft construction
  and commands, and derive stable carried-clone IDs. Reconcile all incompatible previous episodes,
  including future/previously ended rows. Materialize only selected missing effective assignments.
- Preserve history readers. Switch actual dated membership/readiness consumers and source guards
  to their owner projections. Include carried future offering activation and stop-before-start.
- Capture and reconcile exact preservation fingerprints and a durable audit in the migration;
  refuse malformed/unmappable inputs atomically. Test fresh, populated, refusal and retry fixtures.

## 3. Publication operations and typed provider boundary

- Delivery `services/change_sets.rs`: default references to Bangkok today; latest-number base;
  separate stale-base/date/consumption findings; locked receipt replay; sequence allocation and
  effectivity reconciliation inside the existing atomic publication/Assessment/audit transaction.
- Delivery `services/versions.rs`: typed latest and dated publication APIs on caller connection/
  transaction, higher-number interval end, list/detail chronology and shared allocation projection.
  Update `services/audiences.rs`, workspace/default selections and dated operational consumers.
- Timetable `services/timetable_lifecycle.rs` and version service: own sequence/replay semantics,
  same-day publication, highest-number dated resolution and source applicability for new publishes.
  Keep commit permits, source-adoption reconciliation and pinned historical reads.
- Timetable Attendance spans: split at effective-date boundaries and choose the highest eligible
  number in each span; omit empty superseded spans while retaining exact frozen source IDs.
  Lifecycle latest planning imports/fingerprints use the owner provider instead of raw date order.
- Keep existing lifecycle, management policies, row revisions and runtime DML privileges. Add
  typed consumed-history refusal where safety cannot be established; retain active no-retroactive
  behavior and no automatic write retries.

## 4. Public contract and frontend handoff

Add nullable `publicationNo` to Delivery versions/summaries, target-derived change-set detail/
summaries and timetable versions. Add publication actor/time to summaries. Remove obsolete
timetable date-collision findings and expose the typed consumed-history finding. Inputs remain
the existing explicit-date, preview-hash, revision and idempotency shapes.
Add the truthful `superseded` timetable display state for an empty logical interval, classify it
before upcoming/current and prevent it from becoming the default. Keep explicit history reads.

Backend regenerates `contracts/openapi/school-api.json` and
`frontend-school/src/lib/api/generated/` through `npm run generate:api-contracts`; never hand-edit
generated output. Notify Frontend when the agreed contract is available. Frontend presents number,
chosen effective date and actual publication time separately, removes date-increment assumptions,
keeps explicit draft source adoption and invalidates previews after candidate-date changes.

## 5. Focused regression and verification

First add reproductions that fail under old semantics. Existing tests requiring strictly later
dates or date-collision refusal change because the approved behavior changes; replace those
expectations with same-day/earlier replacement and stable-pin assertions, retaining stale, scope,
readiness and atomicity coverage. Do not weaken unchanged behavior expectations.

Select these focused owning commands on stable inputs, using the Senior-assigned shared Cargo
slot and existing toolchain/cache. Exact narrower filters may be used during implementation;
QA independently confirms final coverage and adds affected consumers where investigation requires.

```bash
git diff --check
git status --short
./scripts/pipeline verify --scope documentation
cargo fmt --manifest-path backend-school/Cargo.toml --all -- --check
cargo check --manifest-path backend-school/Cargo.toml -p school-academic-delivery -p school-academic-timetable -p school-academic-lifecycle -p school-academic-assessment --locked
cargo check --manifest-path backend-school/Cargo.toml -p school-academic-http --features api-docs --locked
./scripts/test_backend_school.sh --integration delivery_versions -- --test-threads=1
./scripts/test_backend_school.sh --integration delivery_draft_lifecycle -- --test-threads=1
./scripts/test_backend_school.sh --integration timetable_clone_runtime_permissions -- --test-threads=1
./scripts/test_backend_school.sh integration_tests::academic::delivery::services_tests -- --test-threads=1
./scripts/test_backend_school.sh integration_tests::academic::services::timetable_ -- --test-threads=2
./scripts/test_backend_school.sh integration_tests::academic::lifecycle::services::term_preparation_tests -- --test-threads=1
./scripts/test_backend_school.sh --package school-academic-delivery teacher_assignments -- --test-threads=1
./scripts/test_backend_school.sh --package school-academic-assessment -- --test-threads=4
```

Run default and `api-docs` production panic guards and architecture checks through the relevant
owning verification commands when the affected package/consumer graph is stable. Run all database
cases against runner-owned disposable PostgreSQL 18.6, including actual DML-only runtime-role
publication and effectivity evidence. No production database or provider branch is a fixture.

From `frontend-school`, after Backend owns the generation slot:

```bash
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
npm run check
```

Frontend runs scoped formatting/lint and affected opening/timetable Playwright workflows against
a ready local preview, including same-day and earlier replacement, explicit adoption, date-change
preview invalidation, readonly/denied states, desktop/mobile and light/dark. Mock fixtures must use
the generated response shape. Frozen Attendance/grade/result source preservation is asserted in
the owning database fixtures, not inferred from a browser mock.

Required transaction cases: matching concurrent replay returns original IDs/number with one audit;
different payload under the same key conflicts; distinct publishers serialize with stale-base
refusal; forced Assessment, ledger, audit and terminal-write failures leave no partial sequence,
snapshot, episode, offering or confirmation changes. Migration cases independently compare UUIDs,
original dates, timestamp ties, graph contents, receipts and downstream pins and prove failed
reconciliation leaves the pre-migration state intact.

## 6. Canonical docs, freeze and delivery

Update `.rules` to distinguish latest publication-number planning membership from dated eligible
membership and document deterministic carried episode identities. Add executable recipes to
`docs/TESTING.md` and coordinated migration/recovery guidance to `docs/OPERATIONS.md`.
State that legacy November dates were preserved and live intended-date evidence was not available.

At meaningful checkpoints, stop all writers/source-mutating tools and acknowledge the Senior's
barrier. Deliver owned paths, actual checks/results, problems, inputs and remaining work without
staging or committing. After the complete candidate freezes, Reviewer checks the integrated diff
and QA independently assesses behavior and runs selected checks. Repair assigned findings, freeze
again and repeat affected acceptance. The Senior owns latest-main reconciliation, PR readiness,
eligible integration and any selected maintenance/migration/deployment acceptance.
