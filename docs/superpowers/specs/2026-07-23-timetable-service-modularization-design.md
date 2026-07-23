# Timetable Service Modularization Design

**Date:** 2026-07-23
**Status:** Approved
**Scope:** Structural refactor of `backend-school` timetable services

## Context

`backend-school/src/modules/academic/services/timetable_service.rs` contains about 2,620 lines.
It is the shared source of truth for admin, self, and parent timetable views and also owns entry
validation, mutations, instructor assignment, move/swap behavior, occupancy, and batch creation.
Its callers already use a stable `timetable_service::...` boundary.

This phase reorganizes the implementation without changing the HTTP API or timetable behavior.

## Goals

1. Separate timetable reads, validation, mutations, instructor behavior, moves, occupancy, and
   batch workflows into independently understandable modules.
2. Preserve the public service surface used by academic, self, parent, realtime, and related
   services.
3. Preserve transaction, conflict-detection, optimistic response, and WebSocket sequencing
   behavior.
4. Strengthen characterization coverage before moving conflict-sensitive workflows.

## Non-goals

- No API, DTO, permission, frontend, WebSocket protocol, schema, or index change.
- No auto-scheduler behavior; manual timetable editing remains the only construction workflow.
- No query rewrite or request-fan-out optimization in this phase.
- No new cache or repository abstraction.

## Selected architecture

`academic/services/timetable_service.rs` remains the public facade.

```text
backend-school/src/modules/academic/services/
├── timetable_service.rs
└── timetable_service/
    ├── shared.rs
    ├── entries.rs
    ├── validation.rs
    ├── instructors.rs
    ├── moves_and_swaps.rs
    ├── occupancy.rs
    └── batch_mutations.rs
```

No `mod.rs` file is introduced.

### `timetable_service.rs`

- Declares private child modules.
- Re-exports all existing public functions and caller-facing helper types.
- Contains no SQL or end-to-end business workflow.

### `shared.rs`

- Owns low-level pure slot, day, period, entry-type, and conflict primitives used by multiple
  workflows.
- Owns only genuinely shared row/context types.
- Does not depend on another child module.

### `entries.rs`

- Lists entries for the shared admin/self/parent data source.
- Fetches and hydrates one entry.
- Creates, updates, and deletes individual entries.
- Resolves classroom-course semester context.

### `validation.rs`

- Validates proposed entries and move candidates.
- Owns classroom, room, teacher, day, period, and semester conflict rules.
- Exposes narrow validation/context functions to entry, move, and batch workflows.

### `instructors.rs`

- Adds and removes timetable-entry instructors.
- Restores or hides instructors for an activity slot or slot period.
- Loads the current user's activity relation for an entry.
- Preserves team-teaching and primary-instructor synchronization rules.

### `moves_and_swaps.rs`

- Swaps two entries and validates proposed moves.
- Preserves lock ordering, atomicity, and conflict result details.
- Uses validation primitives rather than duplicating conflict SQL.

### `occupancy.rs`

- Loads semester occupancy used for client-side move validation.
- Owns compact occupancy row mapping and instructor aggregation.

### `batch_mutations.rs`

- Creates validated timetable entries in bulk.
- Deletes entries by slot, batch request, or batch group.
- Preserves count reporting, deduplication, conflict summaries, and transaction behavior.

## Dependency and visibility rules

All external callers continue to use `timetable_service::...`. Child modules are private.
`entries`, `moves_and_swaps`, and `batch_mutations` may call narrow `pub(super)` validation
operations. `occupancy` is read-only and does not mutate timetable state. Realtime broadcasting
remains in handlers; services return the same outcomes and sequence values as before.

The three view paths remain distinct authorization boundaries over one service data source:

```text
/api/me/timetable
/api/academic/timetable
/api/parent/students/{id}/timetable
                     -> timetable_service::list_entries(...)
```

## Behavioral compatibility

- Public function paths and signatures remain unchanged.
- SQL and bind ordering are preserved during extraction.
- Row locks and transaction boundaries remain unchanged.
- Conflict outcomes and HTTP mappings remain unchanged.
- WebSocket patch sequence handling remains unchanged.
- Manual drag/drop, batch operations, templates, realtime, and exports retain current behavior.
- No scheduler-only schema or behavior is reintroduced.

## Testing strategy

Before extraction, focused PostgreSQL characterization tests cover:

- filtered entry listing for classroom, instructor, self, and parent use;
- create, update, and delete with persisted instructor/room state;
- classroom, room, and instructor conflicts;
- add/remove/restore/hide instructor behavior;
- swap and move validation, including rollback on conflict;
- semester occupancy hydration;
- batch create/delete counts, skipped/blocked rows, and batch-group deletion;
- lock-sensitive concurrent mutations where current behavior relies on serialization.

Pure conflict and mapping tests move beside their owning modules. Static architecture tests protect
the public facade, child privacy, shared list source, and absence of SQL in the facade.

## Implementation sequence

1. Record the service surface and add missing database characterization tests.
2. Extract shared primitives and read-oriented occupancy.
3. Extract entry reads and individual mutations.
4. Extract instructor workflows.
5. Extract validation and move/swap orchestration.
6. Extract batch mutation workflows.
7. Reduce the facade to declarations and re-exports.
8. Tighten visibility and run full backend, contract, and realtime guards.

## Definition of done

- `timetable_service.rs` is a thin private-module facade.
- All admin, self, parent, and related service callers retain their existing paths.
- Critical conflicts, transactions, and batch outcomes have database characterization coverage.
- No child file becomes a replacement monolith.
- No migration, API, permission, frontend, or generated-contract change occurs in this phase.
- Formatting, strict Clippy, static architecture tests, and the full PostgreSQL-backed backend
  suite pass.
