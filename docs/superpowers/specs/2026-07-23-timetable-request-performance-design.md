# Timetable Request Performance Design

**Date:** 2026-07-23
**Status:** Approved
**Scope:** Timetable activity-context batching, stale-request protection, and evidence-based query
optimization

## Context

The timetable page already batch-loads course instructor teams, but activity data still has
request fan-out:

- classroom mode loads classroom assignments once per independent activity slot;
- instructor mode loads synchronized slot instructors sequentially once per slot;
- instructor mode then loads classroom assignments sequentially once per independent slot.

The number of browser requests therefore grows with the number of activity slots. Rapid changes
to semester, classroom, instructor, or view mode may also allow an older response to commit after
a newer selection because the API client has no `AbortSignal` support.

## Goals

1. Replace activity-slot request fan-out with one typed semester-scoped endpoint.
2. Keep the number of backend queries fixed with respect to slot count.
3. Cancel obsolete timetable loads, deduplicate identical in-flight loads, and prevent stale state
   commits.
4. Update generated OpenAPI and TypeScript contracts from the Rust source of truth.
5. Add a timetable index only when representative `EXPLAIN (ANALYZE, BUFFERS)` evidence shows a
   real benefit.

## Non-goals

- No caching service, Redis, Prometheus, Grafana, or new runtime process.
- No automatic scheduling or scheduling configuration.
- No visible timetable workflow, permission, WebSocket protocol, or optimistic-mutation change.
- No retry of mutations and no global retry policy.
- No speculative index that duplicates an existing useful access path.

## Backend API

Add:

```text
GET /api/academic/activity-slots/timetable-context?semester_id={uuid}
```

Operation ID:

```text
getActivitySlotTimetableContext
```

The handler uses `actor_tenant_context`, requires the existing academic course-plan read access
used by the timetable page, and delegates to the academic activity service. It returns the normal
`ApiResponse<T>` envelope.

The typed response is:

```rust
pub struct ActivitySlotTimetableContextResponse {
    pub slots: Vec<ActivitySlot>,
    pub instructors_by_slot: HashMap<Uuid, Vec<SlotInstructorInfo>>,
    pub classroom_assignments_by_slot: HashMap<Uuid, Vec<SlotClassroomAssignment>>,
}
```

All fields serialize in camelCase. UUID map keys serialize as strings in JSON. An empty semester
returns empty collections rather than `404`.

The service performs a fixed set of semester-scoped queries:

1. load activity slots and their participating classroom IDs;
2. load every slot instructor for those slots in one query;
3. load every classroom assignment for those slots in one query.

Rows are grouped in Rust by slot ID. Any query failure fails the whole response; the endpoint does
not return silently incomplete context.

Existing per-slot endpoints remain available for management screens and mutations.

## Frontend API and request coordination

Add a generated-schema-backed `getActivitySlotTimetableContext()` function to the academic API
client. The generic API client accepts:

```ts
export interface ApiRequestOptions {
    signal?: AbortSignal;
}
```

GET helpers forward the signal to `fetch`. Existing callers remain source-compatible because the
options argument is optional.

Add `frontend-school/src/lib/utils/request-coordinator.ts` with a framework-independent
coordinator:

```ts
export interface RequestCoordinator {
    run<T>(
        scope: string,
        key: string,
        operation: (signal: AbortSignal) => Promise<T>
    ): Promise<T>;
    abort(scope: string): void;
    abortAll(): void;
}
```

Behavior:

- the same `scope` and `key` reuse the existing in-flight promise;
- a different key in the same scope aborts the previous request before starting the new one;
- completion removes only the matching in-flight record;
- callers can check a shared `isAbortError()` helper;
- `abortAll()` is called when the timetable page is destroyed.

The timetable page loads activity context once for the selected semester and derives classroom
and instructor sidebar data from the returned maps. It no longer calls
`listSlotInstructors()` or `listSlotClassroomAssignments()` inside a loop.

Timetable entries, occupancy, classroom courses, course teams, activity context, and instructor
groups are loaded in parallel when their required selection inputs are available. Each state
commit verifies it belongs to the current request key. An aborted request produces no toast;
non-abort failures retain the current user-visible error behavior.

## Database query analysis and index policy

Measure the timetable list, semester occupancy, and activity-context queries against a
representative PostgreSQL fixture using:

```sql
EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
```

The first candidate is:

```sql
CREATE INDEX idx_timetable_active_semester_slot
ON academic_timetable_entries (
    academic_semester_id,
    day_of_week,
    period_id
)
WHERE is_active = true;
```

Add this index in a new sequential migration only if the representative plan shows that the
existing separate semester and day/period indexes cause avoidable scanning or bitmap work and the
candidate reduces execution work for list/occupancy queries. If the existing indexes are already
appropriate, commit the recorded plan comparison and do not add a redundant migration.

Applied migrations, including `001_baseline.sql`, are immutable.

## Error and security behavior

- Tenant and actor context are resolved on the backend; the client cannot select another tenant.
- The batch endpoint applies the existing timetable read permission and activity resource
  constraints.
- Query errors propagate as `AppError`.
- `AbortError` represents local cancellation, not an application failure.
- No request body, credential, token, national ID, or other PII is logged.
- No partial response is cached or reused after failure.

## Testing strategy

Backend tests cover:

- required permission and tenant isolation;
- empty-semester response;
- slots, instructors, and classroom assignments grouped under the correct slot IDs;
- fixed batch query implementation with no per-slot database loop;
- normal API envelope and generated OpenAPI operation.

Frontend tests cover:

- same-key in-flight deduplication;
- different-key cancellation;
- stale completion cannot clear or replace a newer request;
- `abortAll()` cancellation;
- timetable context mapping for classroom and instructor modes;
- aborted loads do not display an error;
- a static guard rejects per-slot instructor/assignment API calls inside the timetable page.

The Svelte timetable component is checked with the official Svelte autofixer and `svelte-check`.
API contract generation and drift tests must pass.

## Implementation sequence

1. Add failing backend contract/service tests for the new context endpoint.
2. Implement the typed service, handler, route, and OpenAPI registration.
3. Regenerate the OpenAPI document and frontend TypeScript schema.
4. Add failing request-coordinator tests, then implement cancellation and deduplication.
5. Add failing timetable resource-mapping/static tests.
6. Replace per-slot frontend calls with the batch context and coordinated parallel loading.
7. Capture representative query plans and add a new index migration only if the stated evidence
   gate is met.
8. Run backend, contract, frontend, Svelte, and migration verification.

## Definition of done

- Activity context requires one browser request per semester load rather than one request per slot.
- Backend query count for the context is constant with respect to slot count.
- Rapid filter changes cannot commit stale activity/timetable state.
- Existing per-slot management APIs and visible timetable behavior remain intact.
- OpenAPI and generated TypeScript include the new operation without unrelated drift.
- Any new index is justified by captured representative plans and added through a new migration.
- Full backend/frontend verification passes without warnings or skipped contract checks.
