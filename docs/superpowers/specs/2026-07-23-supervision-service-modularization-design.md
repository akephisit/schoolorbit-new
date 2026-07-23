# Supervision Service Modularization Design

**Date:** 2026-07-23
**Status:** Approved
**Scope:** Structural refactor of `backend-school` supervision services

## Context

`backend-school/src/modules/supervision/services.rs` contains about 4,200 lines covering cycle
configuration, rubric templates, observation requests, evaluator assignment, evaluation
submission, review and approval, reporting, and supporting database hydration. The handlers
already call a stable `supervision::services::...` boundary, but the implementation file is too
large to change or review safely as one unit.

This phase applies the compatibility-facade pattern established by the exam-schedule refactor.

## Goals

1. Split the service by cohesive supervision workflows.
2. Preserve all existing public Rust paths, types, SQL behavior, transaction boundaries, errors,
   permissions, and HTTP responses.
3. Add PostgreSQL characterization coverage before moving high-risk workflow transitions.
4. Keep business rules independently testable with focused unit tests.
5. Leave a thin facade that prevents callers from depending on implementation modules.

## Non-goals

- No API, DTO, permission, migration, frontend, or user-flow changes.
- No query optimization, cache, retry, or observability work.
- No new repository framework or mocking layer.
- No changes to supervision workflow rules or status names.

## Selected architecture

`modules/supervision/services.rs` remains the only supported public service entry point. It
declares private child modules and re-exports the same functions and public helper types consumed
by handlers and policies.

```text
backend-school/src/modules/supervision/
├── services.rs
└── services/
    ├── shared.rs
    ├── cycles.rs
    ├── templates.rs
    ├── observations.rs
    ├── evaluations.rs
    └── reviews_and_reports.rs
```

No `mod.rs` file is introduced.

### `services.rs`

- Declares private child modules.
- Re-exports the existing caller-facing functions and types.
- Contains no SQL, transaction, or business workflow.
- Keeps `SupervisionObservationListAccess` available at its current path for the access policy.

### `shared.rs`

- Owns pure status-transition, target-selection, score, result-visibility, and evaluator-completion
  rules used by multiple workflows.
- Owns only database row types used by more than one child module.
- Does not own an end-to-end database workflow or become a general utility module.

### `cycles.rs`

- Lists, loads, creates, and updates supervision cycles.
- Owns target normalization and cycle-target persistence.
- Keeps cycle configuration transactions and bulk target writes unchanged.

### `templates.rs`

- Lists, loads, creates, and updates rubric templates.
- Owns section, item, and workflow-step hydration.
- Preserves validated bulk writes for sections, items, and steps.

### `observations.rs`

- Lists and loads observations.
- Loads evaluator availability and timetable choices.
- Creates and edits teacher requests.
- Updates, cancels, approves, or returns observation requests while preserving current transition
  and audit ordering.

### `evaluations.rs`

- Replaces evaluator assignments.
- Submits evaluator responses.
- Preserves conflict checks, required-evaluator rules, response validation, and bulk upserts.
- Exposes narrow `pub(super)` hydration helpers needed by review responses.

### `reviews_and_reports.rs`

- Loads observation review details.
- Certifies, academically approves, and acknowledges results.
- Produces cycle progress and teacher-status reports.
- Uses shared result-visibility and evaluator-completion rules without duplicating them.

## Dependency and visibility rules

External callers use only `supervision::services`. Child modules are private and may expose the
smallest required `pub(super)` helper to sibling workflows. `shared.rs` does not depend on a
use-case module. Database row structs stay with the module that executes the query unless two or
more siblings genuinely share the row.

Handler behavior remains:

```text
request context -> resource policy/permission -> service facade -> response
```

Services do not depend on Axum request or response types.

## Behavioral compatibility

- Public function names, parameters, return types, and re-export paths remain unchanged.
- SQL text and bind order remain unchanged during extraction.
- Transactions begin and commit at the same service boundaries.
- Bulk inserts and upserts remain bulk operations.
- Audit/action rows retain their current transaction and ordering.
- Error variants, status transitions, user-visible messages, and result visibility remain
  unchanged.
- A behavioral defect discovered during extraction is reproduced and fixed in a separate focused
  change, not silently changed in a move commit.

## Testing strategy

Before extraction, PostgreSQL characterization tests cover:

- cycle and target lifecycle;
- template section/item/step persistence;
- teacher request creation, edit, cancellation, approval, and return;
- evaluator replacement, including submitted evaluators and conflict handling;
- evaluation response validation and bulk persistence;
- certification, academic approval, acknowledgement, and result visibility;
- cycle progress and scoped teacher-status reporting;
- rollback when any multi-row workflow step fails.

Pure tests move beside the child module that owns the rule. Static architecture tests protect the
facade surface, private modules, bulk mutation helpers, and absence of SQL in the facade.

## Implementation sequence

1. Record the public surface and add missing characterization tests.
2. Extract shared pure rules and shared types.
3. Extract cycles and templates.
4. Extract observation request and lifecycle workflows.
5. Extract evaluator assignment and evaluation submission.
6. Extract review, approval, acknowledgement, and reporting.
7. Reduce the facade to declarations and re-exports.
8. Tighten visibility and run the full verification suite.

Each extraction is a separate commit and must pass its focused tests before the next extraction.

## Definition of done

- `services.rs` is a thin compatibility facade without SQL or business workflow.
- Existing handlers and policies compile without importing a child implementation module.
- No child service becomes a replacement monolith; files stay cohesive and generally below about
  1,000 lines.
- Characterization and focused unit tests cover the critical workflow boundaries.
- API, permissions, migrations, frontend behavior, and generated contracts are unchanged.
- Formatting, strict Clippy, static architecture tests, and the full PostgreSQL-backed backend
  suite pass.
