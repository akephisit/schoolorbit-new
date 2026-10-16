# Permanent student register

Senior-agreed working agreement, using the recommended administrative-status default shared in chat: the student directory is a permanent register, independent of the
selected academic year. Academic Core remains the owner of annual enrollment, class placement,
class numbers, participation and reviewed academic outcomes.

## Data and status ownership

Students owns identity, student code, registration date and administrative registration status.
The status values are `unrecorded`, `pending`, `studying`, `transferred_out`, `withdrawn` and
`graduated`. This is the school's administrative assertion, not an attendance eligibility rule
or the academic graduation decision. Changing it never changes account access, enrollments,
placements or rosters. Re-entry never resurrects old annual records. Existing academic departure
and graduation workflows retain their existing ownership and permissions.

A forward migration adds the status and its modification timestamp/actor to `student_info`.
Existing rows become `unrecorded` with null provenance: historical transfers cannot be inferred
from accounts or annual records. The database default then becomes `pending` for all future
registrations, including Admission inserts. Manual creation records the authenticated actor and
creation timestamp; updates record provenance only when the status actually changes. Existing
student codes, enrollment dates, identities, relationships and academic history are preserved.

## API and authorization

`GET /api/students` lists the entire authorized register without annual joins or a default account
status filter. Its fields are identity, student code, existing nullable `enrollment_date`,
`registration_status` and `account_status`. Optional queries are `search`, `registrationStatus`,
`accountStatus`, `page` and `pageSize`; sorting is given name, surname and stable user ID before
pagination. Nullable usernames and missing student-info rows remain visible without invented
identities; a missing student-info record rejects status/date writes transactionally rather than
inventing a student code. Academic grade, homeroom and class number are absent from this permanent list.

Staff `GET /api/students/{id}` returns a flat `StudentRegisterProfile` with an explicit
`account_status` and administrative status/provenance. Its optional `academicYearId` projects
only that year's grade, room and class number; absent enrollment does not hide the permanent
identity. No year means these annual fields are null. Required-year self-service and parent
profiles retain their existing DTOs and contracts.

Existing school, own and assigned profile permissions remain authoritative. List own/assigned
permissions form a union. Assigned means an active student-year with a current placement and
matching homeroom advisor, matching the profile policy. PII permission remains separate and
protected fields fail closed on decryption failure. No new permission grants are introduced.

Manual create/update accept administrative status. Creation defaults to pending. Student code
is immutable after creation. Permanent forms never write a class number. Optional update values
have absent/keep, null/clear and value/set semantics. Backend validation trims names, enforces
column lengths, validates dates/email/gender and uses the shared non-truncating password rule.

## Staff workflow

The directory and new/edit flows work without an academic-year prerequisite. Default filters show
all registrations and account statuses. Old annual/account query links are normalized by the UI.
Student details preserve the valid origin: a register link returns to the register; an annual
student page preserves its selected year and return destination. Academic history is an
independently authorized region and never blocks identity loading.

New/edit forms use shared compact controls and field sections in one column on mobile and
sensible grouped columns on desktop. Common fields are visible without multiple large cards.
Optional guardian entry can expand and keeps its unsaved draft while toggled off. Explicit field
errors, submit failures, permission visibility and unsaved edits remain accessible. Status labels
explain administrative registration versus account access and annual academic participation.

## Verification and delivery

Backend checks cover migration preservation/defaults, unenrolled students, all accounts, sorting
and pagination, scoped unions and denials, selected-year projection, PII masking/failure, tri-state
updates, validation, transactional rollback and unchanged annual history. Existing self/parent
selected-year behavior remains covered. Generated OpenAPI and TypeScript must match the new DTOs.
Frontend checks cover URL/context behavior, compact forms, state disposal/retry and responsive
light/dark workflows using synthetic fixtures. Independent Reviewer and QA assess the frozen
candidate. The Senior owns shared Git, PR delivery, eligible integration and selected Pipeline
acceptance under the repository workflow and existing user authorization. Delegated writers do not
perform Git or production operations independently.

## Rollout and recovery

The migration is additive and preserves all previous columns and rows. A previous backend and
frontend pair can operate against the expanded schema while preserving new registration data.
The new list query/response and staff detail contracts are incompatible with old consumers, so
mixed producer/consumer versions are not supported and no runtime compatibility bridge is added.
Use the existing serialized maintenance release to deploy both selected applications and complete
asset/browser acceptance. A failed partial release keeps maintenance enabled and follows the
existing Operations recovery flow to restore a compatible pair or roll forward. Do not remove
new registration data when recovering the application pair.
