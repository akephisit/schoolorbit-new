# Role and Organization Unit Soft-Deactivation Design

**Date:** 2026-07-22

**Status:** Approved

**Scope:** Resolve improvement item M-6 by making role and organization-unit deletion reversible, closing inactive-authorization gaps, and documenting the new mutation contracts as the first Phase 4 API-contract batch

## Context

The frontend currently exposes `DELETE /api/roles/{id}` from the role detail page and defines a `DELETE /api/organization/units/{id}` API helper, but backend-school registers neither route. Adding empty handlers or OpenAPI-only paths would make the contract appear complete without defining safe behavior for records that are referenced throughout authorization and organization history.

Both `roles` and `organization_units` already have an `is_active` column. Their management models and update requests expose the field, but current list services return only active records. More importantly, the effective-permission query does not join either table when resolving role-based and organization-derived permissions. Setting `is_active = false` therefore hides a role or unit from selected lists without reliably stopping its permissions.

Physical deletion is unsafe:

- deleting a role cascades through `role_permissions` and `user_roles`, destroying assignment history;
- deleting an organization unit cascades through members, permission grants, and scoped delegations, while user-role organization references become null;
- the root school unit and bootstrap administrator role are required system records;
- existing audit support is not yet integrated into these status mutations.

M-6 will consequently be resolved as a domain-level soft-deactivation feature rather than as two thin delete handlers.

## Goals

- Make `DELETE` for roles and organization units mean reversible deactivation.
- Stop inactive roles and units from contributing effective permissions immediately.
- Preserve assignments, membership, grants, delegations, and historical references.
- Prevent deactivation of protected system records.
- Prevent inconsistent active organization trees.
- Ensure `DELETE` and `PUT ... is_active=false` use the same domain rules.
- Keep management screens able to discover and reactivate inactive records.
- Keep assignment and lookup screens active-only by default.
- Invalidate permission caches and notify connected clients after an effective status change.
- Audit deactivation and reactivation without storing sensitive payloads.
- Add backend OpenAPI metadata, generated frontend types, and drift protection for the new operations.
- Establish the first bounded mutation-contract batch before proceeding through the rest of Phase 4.

## Non-goals

- Physically delete roles, organization units, assignments, memberships, grants, or delegations.
- Rename or reorganize the generated permission registry.
- Introduce separate organization CRUD permissions in this batch; the existing `roles.*.all` permissions continue to govern role and organization administration.
- Redesign all organization hierarchy mutations or all access-control SQL in unrelated feature modules.
- Add bulk deactivation or recursive subtree deactivation.
- Add a generated runtime HTTP client or runtime schema validator.
- Complete every Phase 4 mutation contract in one unreviewable change.
- Edit an applied migration; all schema changes use a new sequential migration.

## Approaches considered

### 1. `DELETE` performs soft deactivation — chosen

Register the missing delete routes and have them transition `is_active` from true to false. Keep `PUT` as the reactivation path and retain historical relationships. This matches the existing frontend intent and `roles.delete.all` permission while making the action safe and reversible.

### 2. Remove `DELETE` and add a dedicated status endpoint

A `PATCH /status` endpoint would make state transitions explicit, but it would replace the existing frontend action, leave `roles.delete.all` without its intended operation, and add a new contract where the existing resource update already supports `is_active`. It offers insufficient benefit for the migration cost.

### 3. Physical deletion

Hard deletion would rely on current foreign-key cascades and remove assignment and organization history. It also raises lockout and referential-behavior risks. This approach is rejected.

## Domain model

### Protected records

A new sequential migration adds a non-null `is_system BOOLEAN NOT NULL DEFAULT false` column to both `roles` and `organization_units`.

The migration marks at least:

- the bootstrap `ADMIN` role as a system role;
- the root `SCHOOL` organization unit as a system unit.

The flag is authoritative for status-transition protection. It is returned in management response DTOs but omitted from create and update request DTOs, so ordinary API consumers cannot promote or demote protected records. Future system records can be marked by a migration or controlled provisioning code without adding more hard-coded handler checks.

The existing baseline migration remains unchanged.

### State transitions

The valid transition rules are:

| Resource state | Requested action | Result |
| --- | --- | --- |
| active, non-system | `DELETE` | set inactive |
| inactive, non-system | `DELETE` | idempotent success, no second audit/event |
| inactive, non-system | `PUT` with `is_active=true` | reactivate |
| system record | any transition to inactive | domain conflict |
| missing record | any transition | not found |

Role assignments remain present while a role is inactive. Users immediately lose permissions contributed by that role. Reactivating the role makes those preserved assignments effective again.

Organization memberships, grants, and delegations also remain present. Inactive units stop contributing permissions. Reactivating the unit makes preserved relationships effective again, subject to their own date, expiry, and revocation conditions.

### Organization hierarchy invariants

- The system root cannot be deactivated.
- A unit with active direct children cannot be deactivated; administrators deactivate children first.
- An inactive unit can be reactivated only when its parent is active, unless it has no parent.
- Creating or moving an active unit under an inactive parent is rejected.
- This batch does not implement recursive cascade deactivation.

These rules keep every active node connected through an active parent chain without destroying the tree.

## Backend API design

### Delete operations

The following operations are added:

```text
DELETE /api/roles/{id}
DELETE /api/organization/units/{id}
```

Both require authentication and `roles.delete.all`. A successful call returns HTTP 200 with the standard `ApiResponse<EmptyData>` envelope and wording that explicitly says the resource was deactivated, not permanently deleted.

Documented error responses are:

- 401 when authentication is missing or invalid;
- 403 when `roles.delete.all` is missing;
- 404 when the resource does not exist;
- 409 when a protected record or organization hierarchy invariant prevents the transition;
- 500 for an unexpected persistence failure through the standard error envelope.

`AppError` gains an explicit domain-conflict variant mapped to HTTP 409 instead of overloading bad-request or database-duplicate handling.

### Update operations

The existing update operations remain the reactivation path:

```text
PUT /api/roles/{id}
PUT /api/organization/units/{id}
```

Their handlers continue to require `roles.update.all`. When a request changes `is_active` from true to false, it additionally requires `roles.delete.all` and delegates to the same status-transition service used by `DELETE`. This prevents a caller from bypassing deactivation policy through the general update endpoint.

Reactivation requires `roles.update.all`. Updates that omit `is_active`, or submit the current value, retain existing update semantics.

The service layer owns transition detection, protection, hierarchy validation, persistence, and its structured outcome. Handlers remain responsible for permission checks, tenant-wide cache/realtime effects after a real transition, and HTTP formatting.

### Management list visibility

The existing list operations gain an optional query parameter:

```text
GET /api/roles?include_inactive=true
GET /api/organization/units?include_inactive=true
```

The default remains false for compatibility. Management pages opt in to all records so an inactive record remains discoverable and can be reactivated. Assignment components, staff creation/editing, and auth-only lookup endpoints keep their active-only behavior.

The list parameter and `is_system` response field are documented in OpenAPI and flow into generated frontend schemas.

## Authorization behavior

The effective-permission query remains the authorization source of truth but gains active-resource conditions:

1. Role permissions join `roles` and require `roles.is_active = true`.
2. Organization grants join `organization_units` and require `organization_units.is_active = true`.
3. A scoped delegation contributes permission only when its referenced unit is active; a delegation with no organization scope keeps its current behavior.
4. Existing assignment end dates, membership end dates, delegation expiry, and delegation revocation rules continue to apply.

Read-only administrative views may still display inactive historical assignments. They must not be confused with effective authorization.

New role assignment already rejects inactive roles and retains that behavior. Adding a member or creating a scoped delegation for an inactive organization unit is rejected. Permission grants may still be configured on an inactive unit so administrators can prepare it before reactivation; those grants do not become effective until the unit is active.

## Cache and realtime behavior

After a real role or organization-unit status transition, the handler:

1. commits the database transaction and audit record;
2. invalidates the tenant permission cache;
3. sends the existing all-users `permission_changed` event.

Tenant-wide invalidation is chosen because a role can affect many users and an organization unit can affect members, grants, and delegates. It is safer and simpler than trying to enumerate every affected identity.

An idempotent delete of an already inactive record returns success without invalidating again or emitting a duplicate event.

## Audit behavior

Deactivation and reactivation write an audit record in the same database transaction as the status change. Audit failure rolls back the mutation.

The record contains:

- actor user ID;
- action `deactivate` or `reactivate`;
- entity type, ID, and non-sensitive display name;
- old and new `is_active` values;
- a concise description.

It does not store permission payloads, member data, tokens, headers, national IDs, or other personal fields. Existing audit helpers may be extended with a transaction-aware save path; the status service must not perform a second non-transactional audit write after commit.

## Frontend behavior

### Roles

- The role management list requests `include_inactive=true` and continues to show active/inactive badges.
- The destructive action is labelled `ปิดใช้งาน`, not `ลบ`.
- The confirmation explains that assigned users lose the role's permissions immediately, preserved assignments return when reactivated, and the action is reversible.
- An inactive role exposes an `เปิดใช้งาน` action through the existing update operation.
- System roles show their protected status and do not expose a deactivation action.
- Users without `roles.delete.all` cannot deactivate through either the action or the status control.

### Organization units

- The organization management page requests `include_inactive=true`.
- The detail panel exposes deactivate/reactivate actions using the same permission rules as roles.
- The root system unit cannot be dragged into an invalid state or deactivated.
- Backend conflict messages, including active-child conflicts, are surfaced to the user.
- Staff forms and other selection lists continue to receive active units only.

Frontend API wrappers consume generated request/response schemas. No handwritten copy of the changed wire DTOs is introduced.

The two touched Svelte 5 management pages keep their current runes/event style. Existing unrelated `$effect` suggestions in the organization page are not part of this feature and will not be mechanically rewritten.

## OpenAPI and generated-contract integration

The new delete handlers are registered in the school OpenAPI document with unique operation IDs, path parameters, standard envelopes, and exact status codes. Existing role and organization list/update metadata is expanded for the query parameter, `is_system` field, and conflict responses.

Generation updates:

- `contracts/openapi/school-api.json`;
- `frontend-school/src/lib/api/generated/school-api.ts`;
- stable aliases in the handwritten role/staff API modules.

Contract coverage tests stop asserting that these delete paths are absent and instead assert their method, operation ID, schemas, and responses. The route inventory and frontend route guard are updated atomically so runtime routing, OpenAPI, and frontend calls cannot drift.

## Testing strategy

Implementation follows test-driven development.

### Service and domain tests

- active non-system role deactivates;
- already inactive role returns an unchanged/idempotent outcome;
- system role deactivation conflicts;
- role reactivation succeeds;
- organization system root deactivation conflicts;
- organization unit with an active child conflicts;
- organization unit with no active child deactivates;
- reactivation below an inactive parent conflicts;
- missing IDs return not found;
- status and audit change commit atomically.

### Authorization tests

- inactive roles no longer contribute role permissions;
- inactive units no longer contribute organization grants;
- delegations scoped to inactive units no longer contribute permissions;
- unscoped valid delegations retain their behavior;
- reactivation restores preserved, otherwise-valid permission relationships;
- permission cache invalidation prevents stale fills from reviving prior permissions.

### Handler and contract tests

- delete routes are registered and protected by `roles.delete.all`;
- a `PUT` deactivation cannot bypass the delete permission;
- success, not-found, forbidden, and conflict envelopes match OpenAPI;
- `include_inactive` defaults to false and opt-in returns inactive records;
- generated artifacts are deterministic and current;
- the exact route guard contains both runtime delete routes and no phantom path.

### Frontend tests

- management pages opt into inactive records;
- assignment consumers stay active-only;
- wording consistently says deactivate/reactivate and no longer claims permanent deletion;
- protected records cannot expose a deactivation action;
- generated types are used by API wrappers;
- Svelte autofixer, type checking, lint/static tests, and affected browser smoke coverage pass.

Database-dependent tests use the project's documented test database environment. If credentials are unavailable, they are reported as not executed rather than represented as passing.

## Phase 4 rollout after M-6

M-6 is the first Phase 4 subproject because it fixes a real authorization inconsistency while exercising mutation request, response, conflict, permission, cache, audit, frontend, and generated-contract patterns.

Remaining mutation contracts proceed in separate bounded plans:

1. remaining staff and organization mutations;
2. academic structure, course planning, timetable, assessment, and related mutations;
3. admission workflows and document metadata mutations;
4. facilities, calendar, supervision, work/workflow, consent, and other support domains;
5. a final coverage audit and CI guard that rejects any unclassified JSON mutation route.

Each batch starts from a direct runtime-route inventory, verifies current frontend use before declaring a path missing or unused, and keeps the repository compiling with generated artifacts current. Protocol-specific SSE, WebSocket, and binary transfer contracts remain separately classified.

## Expected outcome

After this subproject:

- the two phantom delete calls have reviewed backend behavior;
- deactivation is reversible and history-preserving;
- inactive authorization sources cannot continue granting access;
- administrators can find and reactivate inactive records;
- bootstrap system records and organization hierarchy remain safe;
- permission changes reach cached and connected clients promptly;
- status changes are auditable;
- runtime routes, OpenAPI, generated TypeScript, and frontend usage remain aligned;
- the remaining Phase 4 mutation rollout has a reusable, security-reviewed pattern.
