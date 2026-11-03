# Academic period preparation

The user approved capacity-only homeroom editing in planning and active years,
preparation of unopened terms within active years, actionable readiness evidence,
and annual rollover without outcome or placement business dates.

Core owns memberships, room capacities, bell schedules and their validation.
Lifecycle owns explicit year/term transitions and the replay-safe manual annual
preparation transaction. Consumers select exact academic context and use the
Core-owned SQL temporal predicate and Rust interval operation.

Annual memberships use `placement_scope=academic_year` with no civil start date.
Existing rows remain `dated`, with their dates and identities preserved. Actual
transfers introduce real dated boundaries. Closing a source membership changes
its status without inventing an end date; historical membership remains scoped
to the explicitly selected old year. Current advisor authorization is constrained
to the active year. Migration 154 reconciles all old placement facts and immutable
promotion evidence before accepting the new representation.

Manual annual preparation takes source/target IDs and declared outcomes. It
prepares inactive target enrollment and room membership, retaining replay,
staleness, capacity and stable-identity safeguards. Activation uses context and
statuses for annual memberships. Historical date evidence remains readable.

Capacity edits cannot reduce capacity below current/planned membership and closed
years remain read-only. Bell schedules used by active/closed terms cannot be
mutated; unopened terms may reuse an unchanged schedule or clone a schedule.
Readiness identifies the selected schedule, implicated periods and rooms with
actionable links. No diagnosis claims a live-data cause without evidence.

Delivery requires focused disposable database tests, both Rust feature modes,
generated contracts, frontend type/static/runtime/browser checks and independent
Reviewer and QA acceptance. Latest-main integration and selected deployment gates
remain owned by the coordinator.

The additive schema remains compatible with the old binary only until the first
date-free rollover or annual NULL-start membership is accepted, whichever occurs
first. A date-free run also has nullable historical placement-date evidence.
After that boundary, old binaries must
not serve traffic; recovery rolls forward under the existing maintenance release
workflow. Frontend assets and the new backend ship together under that boundary.
