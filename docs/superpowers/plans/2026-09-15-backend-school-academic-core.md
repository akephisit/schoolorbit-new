# Backend School Academic Core Extraction Plan

> **Execution:** Implement task-by-task with the approved crate-architecture specification and
> `.rules` as the governing standard.

**Goal:** Complete checkpoint 7 by making `school-academic-core` the singular owner of canonical
academic contexts, years, terms, catalogs, curricula, grade/program structures, student-year and
homeroom relationships, intrinsic lifecycle guards, and Core provider operations, while removing
every production dependency from a lower academic area to the lifecycle orchestrator.

**Architecture:** Keep Axum handlers, router registration, OpenAPI composition, migration/cutover
fixtures, and cross-domain integration tests in the application. Move Core models, repositories,
validation, and services into an acyclic crate depending only on authorization, errors,
permissions, and database/data primitives. Move term/year transition and reopening orchestration
up to the application-owned lifecycle module before extracting Core. Lower academic providers use
Core-owned typed port contracts rather than lifecycle-owned models. Lifecycle converts its wire
commands into narrow Core promotion commands and invokes public Core operations directly.

**Constraints:** Keep one executable/image and preserve every route, operation ID, DTO/schema name,
error message, permission, lock order, transaction boundary, audit payload, idempotency receipt,
status transition, migration, and generated artifact. Do not edit applied migrations. Do not add a
root compatibility re-export, production `#[path]`, full `AppState`, generic shared crate, reverse
dependency, or new PII/log output.

## Task 1: Prove the Intended Dependency Direction

- [x] Inventory production and test consumers of Core models/services, every lower-to-lifecycle
  import, lifecycle transition SQL currently stored under Core, lookup DTO ownership, and the
  academic resource-list policy boundary.
- [x] Add RED static guards requiring the `school-academic-core` workspace member, singular Core
  source ownership, direct crate imports, no root/application/lifecycle/lower-domain dependency in
  the crate, and no production lower academic import of lifecycle models or services.
- [x] Extend the workspace dependency allowlist with the exact one-way Core edge and prove the new
  guards fail before source movement.

## Task 2: Establish Lower-Layer Contracts Before Moving Source

- [x] Move `AcademicResourceListFilter`, `AcademicResourceAccess`, their permission description,
  resolver, and pure access decision into `school-authorization`; retain root policy composition
  as a direct consumer and preserve exact organization-unit/tree semantics and tests.
- [x] Move the academic-year and grade-level lookup DTOs to Core ownership; update lookup,
  delivery, OpenAPI, and all other consumers directly so schema names and serialized fields remain
  unchanged.
- [x] Add Core-owned narrow port types for pending work and term-preparation provider inputs and
  outputs. Convert lifecycle models/composition to those types and remove production lifecycle
  imports from delivery, timetable, exam, assessment, results, and Core code.
- [x] Add a Core-owned promotion destination command type. Map lifecycle decisions explicitly at
  the lifecycle boundary so Core validation/execution no longer imports lifecycle wire models.

## Task 3: Correct Transition Ownership

- [x] Move activation, term transition, year transition, and year reopening commands plus their
  request/outcome wire types from Core to the application-owned lifecycle module. Keep Core-owned
  context/evidence projections and intrinsic advisory-lock/state guards below lifecycle.
- [x] Move promotion-policy option/rule orchestration from the Core folder into lifecycle while it
  consumes Core grade/program/progression operations directly.
- [x] Replace Core's lifecycle checksum call with an owned deterministic evidence checksum and
  retain byte-identical hashes. Keep Results' annual-revision guard above Core and preserve every
  transition transaction/receipt/audit ordering.
- [x] Relocate transition and cross-domain tests with their lifecycle owner; keep Core database
  integration tests in the root only when they intentionally exercise students, cutover fixtures,
  or multiple future academic owners.

## Task 4: Extract `school-academic-core`

- [x] Add the workspace crate with centralized dependencies/lints and dev-only test dependencies.
  Move Core models, validation, persistence/services, intrinsic lifecycle guards, evidence readers,
  and focused owner tests; leave the root `academic::core` module as routes, handlers, and explicit
  integration-test ownership only.
- [x] Make only the narrow operations required by root handlers and downstream academic providers
  public. Do not expose database row helpers merely to preserve old module visibility.
- [x] Cut every production and test consumer to `school_academic_core` directly, remove the old
  model/service owner, and keep root handlers thin with the same permission and tenant adapters.
- [x] Add focused package database tests against canonical migrations for representative year/term,
  catalog/curriculum, relationship, lock, and promotion-provider behavior; keep test infrastructure
  dev-only and absent from the normal release graph.

## Task 5: Verify Behavior and Compile Scope

- [x] Run Core package tests, authorization tests, every affected lower/lifecycle focused suite,
  static architecture tests, API contract tests, and the complete disposable PostgreSQL root suite.
- [x] Regenerate/check permission and API contracts and prove tracked permission/OpenAPI/frontend
  artifacts plus both migration directories remain byte unchanged.
- [x] Run frontend lint, type checking, menu/static/documentation suites, build and inspect the
  release image, run actionlint, and confirm the normal release graph contains neither
  `school-test-db` nor any test-support feature.
- [x] Measure three comment-only incremental workspace checks for `school-academic-core`, record
  dirty/fresh package evidence, and compare the median with the 59.445 second checkpoint-1
  local-package baseline. A median above the 10-percent regression gate blocks completion.
- [x] Update the approved master status, backend README, testing documentation, and this checklist
  only after the exact candidate tree passes every runnable gate; explicitly name any unavailable
  environment-dependent check.

## Completion Gate

Checkpoint 7 is complete only when Core has one crate owner and an intentionally narrow public
surface; lower academic production code has no lifecycle dependency; lifecycle owns transitions
and maps its commands downward; root retains only HTTP/composition/cross-domain test concerns;
lock, status, receipt, audit, DTO, OpenAPI, permission, migration, and runtime behavior are
unchanged; all runnable verification passes; and the representative Core edit meets the compile
regression gate.
