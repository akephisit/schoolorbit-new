# Navigation and My Work Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rework the school app navigation foundation so the sidebar stays stable and easy to scan, while temporary department-assigned work appears in a dedicated "My Work" flow. Separate permissions, workflow windows, assignments, and time windows so future school workflows can be added without overloading the permission system.

**Architecture:** Keep route/menu metadata as the source of permanent navigation. Backend filters menu visibility by user type and permission. Add workflow-window and work-item foundations for time-bounded school operations from departments, organization units, or direct users. Frontend consumes typed menu and work APIs, renders a workspace-oriented sidebar, and uses route guards only for coarse access decisions.

**Tech Stack:** SvelteKit 5 CSR app, TypeScript, Tailwind/shadcn-svelte, Rust Axum, sqlx/PostgreSQL, SSE refresh notifications.

---

## Current Decision

Use **Workspace Sidebar + My Work Inbox**.

Permanent modules should not appear and disappear just because a department opens a short-lived task. The sidebar should have stable workspaces such as teaching, academic, student affairs, staff/admin, and system. Dynamic or temporary tasks should appear under "งานของฉัน" with counts, status, deadline, and deep links to the real workflow.

This keeps four concepts separate:

- **Permission:** the actor is allowed to perform an operation.
- **Workflow window:** a responsible unit has opened or closed a real school process, such as score submission, club registration, lesson-plan submission, or document collection.
- **Assignment:** the actor has been asked to do a specific task inside a workflow window.
- **Time state:** the task is open, due soon, closed, submitted, or read-only.

Routine school operations should not use feature toggles. Core systems should be available when the actor has permission. What opens and closes day to day is the workflow window, controlled by the responsible head, department, organization unit, or role with the correct permission.

If a low-level system availability toggle remains later, it is only for rollout, emergency disable, or tenant-level operations. It is not part of normal school workflow management and should not drive the sidebar design.

## Non-Goals

- Do not make every temporary work request a new sidebar module.
- Do not use permission strings as deadlines, workflow windows, or assignment state.
- Do not use feature toggles for normal operations such as opening score submission, club registration, or document collection.
- Do not add legacy compatibility paths for old menu or organization behavior.
- Do not expose PII or full staff/student profiles through lookup endpoints for menu convenience.

## Phase 1: Menu And Permission Contract

- [ ] Extend the backend menu contract with stable workspace metadata.
  - Add typed fields to backend menu DTOs: `workspaceCode` and optional `badgeKey`.
  - Keep response envelope shape `{ success, data, message? }`.
  - Keep handlers thin: request context, permission checks, service call, response formatting.

- [ ] Keep menu filtering permission-based.
  - Hide menu items when the actor lacks the route/module permission.
  - Keep permission checks resource-agnostic at menu level.
  - Direct API calls must still enforce permission, resource scope, and workflow-window state in backend policies/services.

- [ ] Update route metadata sync so real menu routes can declare workspace codes.
  - Frontend `_meta.menu` remains the source for menu item registration.
  - Do not add incomplete `_meta.menu` to child/detail pages only for guarding.
  - Use frontend permission constants from `frontend-school/src/lib/permissions/registry.ts`.

- [ ] Add static tests for menu metadata integrity.
  - Backend: menu DTO/service does not return ad-hoc raw JSON.
  - Frontend: `_meta.menu.permission` uses constants, not raw strings.
  - Frontend: route menu metadata uses known workspace codes.

## Phase 2: Workflow Window And Work Item Foundation

- [ ] Add clean schema for workflow windows.
  - `workflow_windows`: module code, workflow code, title, organization unit scope, managed-by permission, open timestamp, due timestamp, close timestamp, status, metadata, created actor.
  - A workflow window represents a real school process that can be opened or closed by the responsible role.
  - Examples: score submission round, club registration round, lesson-plan submission round, department document collection round.

- [ ] Add clean schema for assigned work.
  - `work_items`: workflow window id, source module, source resource type/id, title, action path, required permission, status, metadata, created actor.
  - `work_item_assignees`: work item id, assignee type, direct user id, organization unit id, position code, status, read/submitted timestamps.
  - Use typed JSONB only when metadata has a known shape; keep generic metadata minimal and non-sensitive.

- [ ] Implement backend workflow-window services.
  - `list_manageable_workflow_windows(pool, actor, filters)` returns windows the actor can manage.
  - `open_workflow_window(pool, actor, request)` validates permission, organization scope, timestamps, and status transitions.
  - `close_workflow_window(pool, actor, window_id)` closes the process without removing historical work.
  - Business rules for open/due/closed/read-only must be pure helpers with unit tests.

- [ ] Implement backend work-item services.
  - `list_my_work_items(pool, actor, filters)` resolves direct and organization-based assignments.
  - `get_my_work_counts(pool, actor)` returns open, due soon, submitted, closed counts.
  - Work item state is derived from assignee status plus the related workflow window state.

- [ ] Implement backend endpoints.
  - `GET /api/me/workflow-windows/manageable`
  - `POST /api/workflow-windows`
  - `PATCH /api/workflow-windows/{id}`
  - `GET /api/me/work-items`
  - `GET /api/me/work-items/counts`
  - Endpoints use `actor_tenant_context`.
  - No handler-owned SQL.

- [ ] Add backend tests.
  - Unit tests for workflow-window timestamp and transition rules.
  - Unit tests for status classification.
  - Unit tests for assignment resolution input normalization.
  - Static architecture guard for the new workflow/work module.

## Phase 3: Frontend Typed API And Store

- [ ] Add typed API client functions.
  - `frontend-school/src/lib/api/work.ts`
  - Concrete DTOs for `WorkflowWindow`, `WorkflowWindowStatus`, `WorkItem`, `WorkItemStatus`, `WorkItemCounts`.
  - No `unknown`, raw `Record<string, unknown>`, or `as ...` casts for known API payloads.

- [ ] Add a small work-item store or loader helper.
  - Sidebar needs counts only.
  - Work page needs filtered item lists.
  - Use silent background refresh for SSE events.

- [ ] Wire SSE refresh.
  - Add or reuse events such as `workflow_window_changed` and `work_items_changed`.
  - Event payload is a signal to refetch typed workflow/work endpoints.
  - Do not send full permission or work payload snapshots over SSE.

## Phase 4: Sidebar UX Refactor

- [ ] Refactor `frontend-school/src/lib/components/layout/Sidebar.svelte`.
  - Keep the component name/path unless the refactor becomes too large.
  - Render stable workspace sections.
  - Put "งานของฉัน" near the top with badge/count and urgent state.
  - Keep existing collapsed-sidebar behavior.
  - Keep keyboard and mobile navigation usable.

- [ ] Reduce duplicate navigation paths.
  - Detail and setup pages should be reachable from their owning module, not repeated as top-level menu items.
  - Organization setup remains under staff/admin workspace.
  - Work assigned by a department appears in "งานของฉัน" and deep-links to the owning workflow.

- [ ] Use shadcn-svelte style patterns consistently.
  - Use real buttons, lists, badges, scroll areas, and separators where available.
  - Avoid card-in-card sidebar layouts.
  - Keep text sizing compact and stable.

## Phase 5: My Work Page

- [ ] Add the main work inbox route.
  - Prefer `/staff/work` first because the current request is staff-side school operations.
  - Keep room for future `/student/work` or shared `/work` if student/parent tasks are added.

- [ ] Implement filters and states.
  - Open
  - Due soon
  - Submitted
  - Closed/read-only
  - No assigned work

- [ ] Implement direct-link behavior.
  - Assigned but closed: show the work item with "ปิดรับแล้ว" or read-only state.
  - Workflow closed: show the relevant closed/read-only state.
  - No permission and no assignment: show `/403`.

## Phase 6: Verification

- [ ] Backend checks.
  - `cd backend-school && cargo test modules::work`
  - `cd backend-school && cargo test modules::workflow`
  - `cd backend-school && cargo test --test static_architecture`
  - `cd backend-school && cargo check`

- [ ] Frontend checks.
  - `cd frontend-school && npm run test:static`
  - `cd frontend-school && npm run lint`
  - `cd frontend-school && npm run check`

- [ ] Browser checks.
  - Staff user with normal permissions sees stable sidebar and no admin-only modules.
  - Admin sees organization/system modules.
  - Staff with assigned work sees "งานของฉัน" badge and work list.
  - Staff without assigned work sees empty "งานของฉัน" state.
  - Closed assigned work does not become a misleading 403.
  - Head/responsible role can open and close only workflow windows they are allowed to manage.

## Commit Plan

- [ ] Commit 1: menu permission contract and static guards.
- [ ] Commit 2: workflow-window schema and backend services.
- [ ] Commit 3: work-item schema, services, and backend tests.
- [ ] Commit 4: workflow/work API endpoints.
- [ ] Commit 5: frontend workflow/work API/store and SSE refresh.
- [ ] Commit 6: sidebar UX refactor.
- [ ] Commit 7: My Work page and route behavior.

## Rollout Notes

- Existing stable menu routes should continue to work after each commit.
- Core module menu visibility should come from permission, not feature toggles.
- Opening and closing school processes should happen through workflow windows managed by authorized responsible roles.
- Work assignment is additive and should not change existing role/permission semantics.
- The first production rollout can ship sidebar and counts before every module creates work items.
