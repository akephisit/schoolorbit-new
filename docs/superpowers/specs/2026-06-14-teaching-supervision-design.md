# Design: Teaching Supervision

## Goal

Build a teaching supervision system for real school operations. The first release supports cycle-based classroom observations tied to the academic year and semester, teacher self-booking from the timetable, customizable school-wide evaluation templates, multiple independent evaluators, approval/signature workflows, and progress reporting.

The system should fit the existing SchoolOrbit architecture:

- SvelteKit CSR frontend with typed API clients and route/menu permission metadata.
- Rust Axum backend with thin handlers, services, resource-aware policies, typed DTOs, and API response envelopes.
- PostgreSQL schema through new sequential migrations after the clean baseline.
- Existing organization, staff, subject group, timetable, permission, and notification foundations.

## Key Decisions

- The first implementation is **cycle-based**. Supervision is organized by academic year and semester, not isolated ad-hoc records.
- The menu is a single real menu route under the `academic` workspace with the Thai title `นิเทศการสอน`.
- Evaluation templates are **school-wide** in the first release.
- Templates are customizable from the start, but only support `rating` and `text` item types in the first release.
- An observation is tied to a real timetable entry whenever possible, with a manual fallback for special lessons or missing timetable data.
- The observed teacher can request/book a supervision slot by selecting only the lesson slot. The teacher does not choose evaluators.
- Actors with `supervision.manage.school` approve booking requests and assign multiple evaluators.
- Multiple evaluators score independently. Numeric rating scores are averaged equally across submitted evaluator responses.
- The template defines the approval/signature workflow.
- The observed teacher signs as `acknowledged` or `acknowledged_with_comment`, not as an acceptance of correctness.
- File attachments are out of scope for the first release.
- The generic `work`/`workflow` foundation is not required for the first release. This module owns its domain workflow directly.

## Domain Model

### `supervision_cycles`

Represents one supervision round for an academic year and semester.

Fields:

- `id`
- `academic_year`
- `semester`
- `title`
- `description`
- `template_id`
- `booking_opens_at`
- `booking_closes_at`
- `starts_at`
- `ends_at`
- `status`: `draft`, `open`, `closed`, `archived`
- `created_by`
- `created_at`
- `updated_at`

Rules:

- Teachers may request slots only when the cycle status is `open` and the current time is within the booking window.
- Evaluations may occur only within the cycle active window unless a user with `supervision.manage.school` changes the observation.

### `supervision_cycle_targets`

Defines who must be observed and how many approved observations are required.

Fields:

- `id`
- `cycle_id`
- `target_type`: `school`, `organization_unit`, `subject_group`, `staff`
- `target_id`
- `required_observations`
- `priority`
- `created_at`
- `updated_at`

Resolution rule:

When a staff member matches multiple target rules, the most specific target wins:

1. `staff`
2. `subject_group`
3. `organization_unit`
4. `school`

If two rules have the same specificity, the lower `priority` value wins.

### `supervision_templates`

School-wide customizable evaluation forms.

Fields:

- `id`
- `title`
- `description`
- `status`: `draft`, `active`, `archived`
- `rating_min`
- `rating_max`
- `created_by`
- `created_at`
- `updated_at`

Rules:

- Only one active template is needed for simple schools, but cycles store `template_id` so old observations keep their historical template.
- Templates can be archived but not hard-deleted once used by a cycle or observation.

### `supervision_template_sections`

Groups evaluation items into ordered sections.

Fields:

- `id`
- `template_id`
- `title`
- `description`
- `sort_order`

### `supervision_template_items`

Defines questions/criteria.

Fields:

- `id`
- `section_id`
- `label`
- `description`
- `item_type`: `rating`, `text`
- `required`
- `sort_order`

Rules:

- Only `rating` items participate in numeric averages.
- `text` items are stored per evaluator and displayed in reports as comments/notes.

### `supervision_template_steps`

Defines the approval/signature workflow for observations created from a template.

Fields:

- `id`
- `template_id`
- `step_order`
- `step_code`
- `label`
- `actor_kind`: `supervisor`, `observed_teacher`, `permission`, `organization_position`
- `actor_permission`
- `organization_position_code`
- `action_kind`: `submit`, `approve`, `return_for_revision`, `publish`, `acknowledge`, `sign`
- `required`

Initial supported actor kinds:

- `supervisor`: an evaluator assigned to the observation.
- `observed_teacher`: the teacher being observed.
- `permission`: any actor with the configured permission, such as `supervision.approve.school`.
- `organization_position`: reserved for future organization-scoped workflows. It may be stored but does not need full UI support in the first release unless the implementation plan includes it explicitly.

### `supervision_observations`

Represents one planned or completed observation.

Fields:

- `id`
- `cycle_id`
- `observed_staff_id`
- `requested_by`
- `approved_by`
- `template_id`
- `timetable_entry_id`
- `manual_subject_name`
- `manual_classroom_label`
- `manual_room_label`
- `manual_observed_at`
- `manual_period_label`
- `manual_reason`
- `lesson_snapshot`
- `status`
- `requested_at`
- `approved_at`
- `cancelled_at`
- `created_at`
- `updated_at`

Status values:

- `requested`
- `planned`
- `in_progress`
- `evaluators_submitted`
- `under_review`
- `returned`
- `approved`
- `published`
- `acknowledged`
- `completed`
- `cancelled`

Rules:

- Observed teachers may edit or cancel their own observation only while it is `requested`.
- After an observation is `planned`, changes must be made by academic administrators.
- If `timetable_entry_id` is present, the backend stores a stable `lesson_snapshot` so reports do not change when the timetable changes later.
- Manual fallback requires enough lesson detail to display a report and requires a short `manual_reason`.

### `supervision_evaluators`

Lists evaluators assigned to one observation.

Fields:

- `id`
- `observation_id`
- `evaluator_staff_id`
- `role_label`
- `is_required`
- `status`: `assigned`, `draft`, `submitted`
- `submitted_at`
- `created_at`
- `updated_at`

Rules:

- Academic administrators assign evaluators when approving a teacher's booking request.
- The observed teacher cannot assign evaluators.
- An observation may enter review only after all required evaluators submit.

### `supervision_evaluator_responses`

Stores each evaluator's independent answers.

Fields:

- `id`
- `observation_id`
- `evaluator_id`
- `template_item_id`
- `rating_score`
- `text_response`
- `created_at`
- `updated_at`

Rules:

- A response belongs to one evaluator and one template item.
- `rating_score` must be within the template rating range.
- `text_response` is allowed only for text items, unless the implementation plan explicitly adds optional notes for rating items.

### `supervision_actions`

Stores domain action history for workflow/signature and reporting.

Fields:

- `id`
- `observation_id`
- `actor_user_id`
- `actor_staff_id`
- `action_kind`
- `from_status`
- `to_status`
- `comment`
- `created_at`

Action examples:

- `requested`
- `request_cancelled`
- `request_returned`
- `planned`
- `evaluator_submitted`
- `submitted_for_review`
- `approved`
- `returned`
- `published`
- `acknowledged`
- `acknowledged_with_comment`
- `completed`

This table is domain history. Important mutations should also write audit logs once the audit foundation is applied to this module.

## Main Workflows

### Template Setup

1. An actor with `supervision.manage.school` creates a template.
2. The actor creates ordered sections.
3. The actor creates rating/text items.
4. The actor configures workflow steps.
5. The actor activates the template.

Validation:

- An active template must have at least one section and one item.
- Rating templates must have valid `rating_min < rating_max`.
- Workflow steps must include at least one approval or publish path before observed-teacher acknowledgement.

### Cycle Setup

1. An actor with `supervision.manage.school` creates a supervision cycle for academic year and semester.
2. The actor selects an active template.
3. The actor sets booking and active windows.
4. The actor defines target rules.
5. The actor opens the cycle.

Validation:

- Booking dates must be inside or before the active cycle dates.
- Targets must have `required_observations > 0`.
- A cycle cannot be opened without a template and at least one target rule.

### Teacher Booking

1. Teacher opens `นิเทศการสอน`.
2. Teacher selects an open supervision cycle.
3. Teacher selects a lesson from their timetable.
4. If needed, teacher uses manual fallback and provides manual lesson details plus a reason.
5. Teacher submits the request.
6. Observation status becomes `requested`.

Validation:

- A teacher may only request for themselves unless they have administrative supervision permission.
- A timetable booking must resolve to a lesson taught by the teacher.
- Manual fallback must be clearly marked and must not expose PII.

### Booking Approval and Evaluator Assignment

1. An actor with `supervision.manage.school` opens requested observations.
2. The actor checks the lesson detail.
3. The actor assigns multiple evaluators.
4. The actor approves the request.
5. Observation status becomes `planned`.

Validation:

- At least one required evaluator must be assigned.
- The observed teacher cannot be assigned as evaluator for their own observation.
- Actors with `supervision.manage.school` may return or cancel invalid requests.

### Evaluation

1. Each evaluator opens assigned observations.
2. Each evaluator fills rating/text responses independently.
3. Each evaluator submits their own response.
4. When all required evaluators submit, the observation can move to review.
5. Numeric summaries are computed as equal-weight averages across submitted required evaluators.

Validation:

- Evaluators may edit only their own draft responses.
- Submitted evaluator responses are locked unless the observation is returned for revision.
- Rating averages ignore text items.

### Review, Publish, and Acknowledgement

1. Observation follows the template workflow steps.
2. Authorized approvers can approve or return the observation.
3. Once approved, the result is published to the observed teacher.
4. The observed teacher acknowledges the result or acknowledges with a comment.
5. Observation reaches `completed`.

Meaning:

- Acknowledgement means the teacher has seen the result.
- Acknowledgement does not mean the teacher agrees with every score or comment.
- If the teacher disagrees, they can acknowledge with a comment. The comment is preserved in the action history.

## Permissions

Backend permission codes should follow canonical `module.action.scope` naming.

Proposed permissions:

- `supervision.read.own`: observed teacher reads their own published or completed results.
- `supervision.read.assigned`: evaluator reads observations assigned to them.
- `supervision.read.organization_unit`: scoped read for future unit-based access.
- `supervision.read.organization_tree`: scoped read for future organization-tree access.
- `supervision.read.school`: school-wide read/report access.
- `supervision.request.own`: teacher requests their own supervision booking.
- `supervision.manage.school`: manage templates, cycles, targets, and booking approvals school-wide.
- `supervision.evaluate.assigned`: evaluator submits assigned evaluations.
- `supervision.approve.school`: approve/publish supervision results school-wide.

Access rules:

- Menu visibility can use a broad supervision permission such as module-level access or `supervision.request.own`.
- Backend policies must enforce resource access for every observation.
- Frontend route guards are UX only. Backend remains the source of truth.

## Frontend UX

Route:

- `/staff/academic/supervision`

Menu:

- Title: `นิเทศการสอน`
- Workspace: `academic`
- User type: `staff`
- Permission: supervision module or a suitable supervision read/request permission.

Primary page layout:

- Dashboard summary for current cycle progress.
- Tabs or segmented views:
  - `ของฉัน`: teacher bookings and own results.
  - `คำขอจอง`: requested observations for actors with `supervision.manage.school`.
  - `ประเมิน`: observations assigned to the current evaluator.
  - `รอบนิเทศ`: cycle and target management.
  - `แบบประเมิน`: template management.
  - `รายงาน`: progress and score reports.

Visibility:

- Show tabs based on `can` store and backend data.
- Do not duplicate the menu into separate "my supervision" and "manage supervision" routes in the first release.
- Detail/action pages should use `_meta.access` if they need stricter guard-only metadata.

## API Shape

All JSON endpoints return the standard envelope:

```json
{ "success": true, "data": {}, "message": "optional" }
```

Proposed endpoint groups:

- `GET /api/supervision/cycles`
- `POST /api/supervision/cycles`
- `PATCH /api/supervision/cycles/{id}`
- `GET /api/supervision/templates`
- `POST /api/supervision/templates`
- `PATCH /api/supervision/templates/{id}`
- `GET /api/supervision/templates/{id}`
- `POST /api/supervision/observations/requests`
- `PATCH /api/supervision/observations/{id}/request`
- `POST /api/supervision/observations/{id}/approve-request`
- `POST /api/supervision/observations/{id}/return-request`
- `GET /api/supervision/observations`
- `GET /api/supervision/observations/{id}`
- `PUT /api/supervision/observations/{id}/evaluations/me`
- `POST /api/supervision/observations/{id}/evaluations/me/submit`
- `POST /api/supervision/observations/{id}/submit-review`
- `POST /api/supervision/observations/{id}/approve`
- `POST /api/supervision/observations/{id}/return`
- `POST /api/supervision/observations/{id}/publish`
- `POST /api/supervision/observations/{id}/acknowledge`
- `GET /api/supervision/reports/cycles/{id}/progress`

The implementation plan may reduce this list for a smaller first commit sequence, but it should not mix raw response shapes or untyped JSON contracts.

## Reporting

First-release reports:

- Cycle progress by target group.
- Teachers completed vs remaining.
- Observation status counts.
- Average rating by teacher, section, and cycle.
- Evaluator submission status.

Later reports:

- Trends across semesters.
- Comparison by subject group or organization tree.
- Export/PDF.

## Non-Goals For First Release

- File attachments.
- Template per subject group or organization unit.
- Automatic evaluator assignment.
- Work/workflow integration.
- Student/parent access.
- PDF generation.
- Mobile offline mode.
- Advanced question types such as multiple choice, checkboxes, rubrics, or file upload.
- Weighted evaluator scoring.

## Implementation Notes

- Add a new backend module `supervision` using `supervision.rs` plus `supervision/handlers.rs`, `supervision/models.rs`, and `supervision/services.rs`; do not create `mod.rs`.
- Add a policy module such as `policies/supervision_access_policy.rs`.
- Use `actor_tenant_context` in handlers.
- Keep handlers thin: request context, policy check, service call, typed envelope response.
- Put database-facing row structs and SQL in services/models, not handlers.
- Add focused unit tests for status transitions, target resolution specificity, rating average calculation, and request edit/cancel rules.
- Add static guards for the new module if needed so handlers do not own SQL.
- Frontend API lives in `frontend-school/src/lib/api/supervision.ts`.
- Frontend route metadata must use permission constants from `frontend-school/src/lib/permissions/registry.ts`.

## Open Implementation Choices

The design has no unresolved product decisions, but the implementation plan must choose an incremental commit sequence. A practical sequence is:

1. Schema, permission registry, and backend pure logic tests.
2. Backend template/cycle services and endpoints.
3. Backend observation request/evaluator/evaluation services and endpoints.
4. Frontend typed API and route shell.
5. Frontend teacher booking and academic approval flow.
6. Frontend evaluator scoring and acknowledgement flow.
7. Reports and polish.
