# Backend School Final Composition and Verification Plan

> **Execution:** Complete checkpoint 12 against the approved crate-architecture specification and
> `.rules`; do not declare the program complete from checkpoint evidence alone.

**Goal:** Close the backend-school crate program by consolidating the exact dependency guard,
removing or explicitly justifying temporary composition seams, measuring the final graph, keeping
compiler-cache policy evidence-based, and passing the complete runnable release matrix.

**Architecture:** `backend-school` remains the only executable and composition root. Internal
crates own cohesive domain logic and expose narrow APIs; root code owns HTTP/runtime adapters and
deliberate cross-domain integration tests. Production dependencies are an exact acyclic allowlist,
while test-only support remains dev-only and exists only for application-owned integration tests.

## Task 1: Consolidate the Final Dependency and Ownership Guard

- [x] Replace checkpoint-specific partial manifest checks with one exact production dependency
  graph covering every workspace crate, centralized workspace dependencies/lints, and absence of
  any internal dependency on `backend-school`.
- [x] Keep focused semantic assertions for auth, migrations, File Platform, and academic edge
  direction; ensure every admitted satellite owner and every retained root owner is explicit.
- [x] Run the static architecture suite RED/GREEN and `cargo metadata --locked` to prove the final
  graph is complete and acyclic.

## Task 2: Clean Final Composition Seams

- [x] Inventory root re-exports, source-path inclusion, test-support features, empty owners, and
  moved-domain source references. Remove stale production compatibility aliases and duplicate
  owners.
- [x] Retain a test-only facade only when a root integration test intentionally spans owners and
  cannot move without reversing dependencies; guard and document that justification. Confirm the
  normal release graph contains neither `school-test-db` nor enabled test-support features.
- [x] Keep the Calendar notification/scheduler, Lifecycle external provider, Delivery timetable
  consequence, Assessment result-lock, auth/File Platform, cache, and realtime adapters in root;
  they must contain no duplicate domain SQL or model ownership.

## Task 3: Measure Checkpoint 11 and the Final Graph

- [x] Warm the single main Cargo target, then run three numbered comment-only workspace checks for
  each checkpoint-11 owner group: Workflow/Question Bank, Admission, Supervision, Students,
  Staff, and Calendar. Remove every probe and record each median against the 59.445-second
  checkpoint-1 baseline.
- [x] Confirm Cargo rebuilds only the edited crate and actual consumers while unrelated academic
  and satellite crates stay fresh; retain timing output outside the repository.
- [x] Measure final warm checks and local-workspace rebuild checks, emit Cargo timings, and block
  completion on an unexplained comparable median regression above ten percent.

## Task 4: Apply the Evidence-Based Build and Documentation Decision

- [x] Inspect the production Docker/workflow cache path. Enable a Rust compiler cache only if two
  ordinary warm CI builds demonstrate useful hits and lower end-to-end time; otherwise retain
  Cargo/cargo-chef and state that decision accurately.
- [x] Update `.rules` so every new or materially expanded backend capability must evaluate a crate
  boundary and use one when the admission criteria and compile-scope evidence justify it.
- [x] Update the master specification status/measurements, backend README, testing documentation,
  and completed checkpoint checklists without creating a separate completion report.

## Task 5: Run the Complete Program Verification Matrix

- [x] Run every workspace package test and the complete disposable root PostgreSQL suite, plus
  formatting, exact static architecture, and `cargo check --workspace --all-targets`.
- [x] Run permission and API generators/checks/tests and prove generated permission/OpenAPI
  artifacts and both migration timelines are byte unchanged.
- [x] Run frontend lint, environment-backed type check, static and documentation tests; run
  actionlint and build/inspect the production backend image.
- [x] Review `git diff --check`, the full branch diff, branch divergence, release dependency tree,
  disk use, and final status. Record unavailable credential-dependent browser/deployed smoke gates
  explicitly rather than replacing them with weaker evidence.

## Completion Gate

Checkpoint 12 and the full program are complete only when checkpoints 1–12 have singular owners
and the approved acyclic graph; root contains composition/adapters rather than duplicate moved
domain logic; contracts, migrations, PII, locks, caches, notifications, realtime, and deployment
topology are preserved; representative edits prove compile isolation; compiler caching follows
measured evidence; every runnable matrix gate passes on the final tree; and no temporary production
compatibility path, probe, or build artifact remains tracked.
