# Backend School Academic Delivery and Timetable Extraction Plan

> **Execution:** Implement task-by-task with the approved crate-architecture specification and
> `.rules` as the governing standard.

**Goal:** Complete checkpoint 8 by making `school-academic-delivery` and
`school-academic-timetable` the singular owners of their domain logic, with the intentional
dependency direction `core -> delivery -> timetable`, while preserving every Delivery and
Timetable transaction, route, DTO, realtime event, permission decision, and generated contract.

**Architecture:** Keep Axum handlers, router/OpenAPI registration, tenant/session resolution,
WebSocket handshake and connection state, cross-domain lookup composition, and broad cutover tests
in the application. Delivery owns offerings, learning groups, rosters, teacher episodes, change
sets, workspaces, and delivery lifecycle checks. Timetable owns versions, templates, blocks,
conflict rules, publication, synchronization, access-resource resolution, and realtime-domain
messages. Delivery never imports Timetable: same-transaction timetable consequences are invoked
through a Delivery-owned async port implemented by an application adapter backed by the Timetable
crate. Timetable may consume Delivery-owned types and read operations.

**Constraints:** Keep one executable/image and preserve routes, operation IDs, schema names,
serialized fields, error text, permission codes, advisory/row lock order, transaction boundaries,
audits, idempotency, migrations, and generated artifacts. No applied migration, production
`#[path]`, root-package dependency, full `AppState`, global service locator, duplicate SQL owner,
or compatibility namespace may be introduced.

## Task 1: Prove and Prepare the Acyclic Boundary

- [x] Inventory Delivery-to-Timetable calls, Timetable-to-Delivery types, cross-domain lookup and
  facility projections, timetable resource authorization, realtime ownership, and all production
  and test consumers.
- [x] Add RED static guards for both workspace members, exact dependency edges, singular source
  ownership, direct consumers, no root/application dependency, no Delivery-to-Timetable import,
  and no production root copy of moved domain implementation.
- [x] Move the timetable resource filter/resolver into a provider-owned public Timetable policy,
  keeping actor and organization-tree semantics and exact failure behavior.
- [x] Add a Delivery-owned async timetable consequence port for group synchronization and
  change-set draft cloning. Implement it in the application after Timetable extraction; preserve
  the existing transaction and lock scope.

## Task 2: Extract `school-academic-delivery`

- [x] Add the workspace crate with centralized dependencies/lints, a direct Core dependency, and
  dev-only database test support. Move Delivery domain models, validation, repositories, services,
  effective-teacher projection, and focused pure tests.
- [x] Represent staff, homeroom, room, and timetable context as narrow Delivery read projections or
  provider inputs. Leave cross-domain lookup orchestration in the application and do not pass full
  application state.
- [x] Retain only route/handler, policy-adapter, realtime publication, lookup composition, and
  deliberate multi-domain test code under the root Delivery module. Root adapters must be thin and
  must not duplicate Delivery SQL or business rules.
- [x] Update Lifecycle, Results, Assessment, parents, students, lookup, and all remaining consumers
  to use the crate public API directly except where an explicit application adapter is required.

## Task 3: Extract `school-academic-timetable`

- [x] Add the workspace crate depending on Core and Delivery. Move timetable models, version,
  template, block, conflict, synchronization, access policy, publication, and realtime-domain
  service logic with focused tests.
- [x] Keep Axum WebSocket credential handling, connection/task lifetime, broadcast registry, and
  root `AppState` access in the application. Move stable Timetable event/access types below that
  adapter and preserve sequence, replay, cleanup, authorization, and disconnect behavior.
- [x] Implement the Delivery timetable-consequence port in the application by delegating to
  Timetable public transaction operations. Keep application wrappers intentionally narrow and
  guarded; do not expose private database rows solely for old visibility.
- [x] Cut handlers, supervision, parents, API composition, and other consumers directly to the new
  Timetable crate and remove old model/service owners.

## Task 4: Preserve Test and Contract Ownership

- [x] Move focused pure and database owner tests into their crates. Keep migration/cutover,
  WebSocket transport, Delivery-to-Timetable transaction, supervision, assessment, and other
  deliberate multi-owner tests in the root with explicit test-only topology where necessary.
- [x] Add canonical-migration package database tests for representative offering/group/roster and
  timetable version/block/template flows without adding test support to the normal release graph.
- [x] Regenerate API and permission artifacts and prove route/schema/TypeScript/permission output
  plus both migration directories remain byte unchanged.
- [x] Run Delivery, Timetable, Core, authorization, affected root focused suites, static
  architecture, and root no-run compilation before broad verification.

## Task 5: Verify Compile Scope and Checkpoint Candidate

- [x] Measure three comment-only workspace checks for each new crate after one unmeasured warm-up.
  Cargo must rebuild only the changed crate and actual dependents; Results changes must not rebuild
  Timetable and Timetable changes must not rebuild Results.
- [x] Compare each median with the 59.445 second checkpoint-1 local-package baseline; a comparable
  median above the 10-percent regression gate blocks completion.
- [x] Run frontend contract/static/docs/lint/type/build checks, backend formatting, workspace
  all-target checks, warning-denied binary check, actionlint, and runtime image build/inspection.
- [x] Update the master status, backend README, testing documentation, and this checklist only
  after the exact candidate tree passes every runnable gate. The complete disposable root database
  suite may be consolidated with the final checkpoint candidate but must pass before the program
  is declared complete.

## Completion Gate

Checkpoint 8 is complete only when Delivery and Timetable each have one crate owner and narrow
public APIs; Cargo dependencies are acyclic in the approved direction; same-transaction timetable
consequences retain their atomicity through an application adapter; root retains only HTTP,
realtime transport, cross-domain composition, and deliberate integration tests; API, permission,
migration, lock, audit, and runtime behavior are unchanged; and representative edits demonstrate
the intended compile isolation.
