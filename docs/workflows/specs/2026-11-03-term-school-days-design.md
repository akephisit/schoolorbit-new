# Term school days and independent schedule copying

Approved from the user's conversation on 2026-11-03.

Academic Core owns `academic_terms.school_days` and the `schoolDays` term contract.
Each term has its own nonempty canonical weekday set. Existing year values are
backfilled exactly to their terms and retained as `default_school_days` /
`defaultSchoolDays`, an initial preset for a first term only. They never validate
or dynamically change an existing term. Preserve years without terms.

Bell schedules remain reusable year-scoped templates. Standalone editing validates
the seven weekday codes, order and times. A nullable period day set means all
school days of the consuming term; an empty explicit set is invalid. Core owns
normalization and effective-day interpretation. Activation, diagnostics, Timetable
preview and writes use that provider and the exact term preset. Existing saved
Attendance configurations retain their facts; unsaved defaults and a first
calendar-only save derive weekdays from the selected term.

Manual term creation and update add `schoolDays`. Atomic copy-create and copy-into-
unopened-term endpoints accept source term/schedule revisions, target weekdays and
the existing target revision when applicable. They create fresh schedule and period
identities and return `{term, bellSchedule, canEditSchoolDays, canAssignBellSchedule}`.
The setup workspace exposes `schoolDaysEditableTermIds` independently of schedule
assignment capability. Source and historical references stay
unchanged. Never copy grades, placements, attendance, timetable facts or other
operational data. Copying does not activate or publish a term.

Lock transition, parents in UUID order, terms, then the source schedule. Compare
captured revisions under those locks. Copy/reassignment requires a never-activated
planning/ready target without timetable references. Weekdays remain editable on
such unopened targets even when references prevent reassignment, but removing a
referenced block's day is refused. Active, closed or ever-activated terms reject
ordinary weekday edits. All writes and audit evidence commit together. Unknown
transport outcomes are reconciled rather than automatically retried.

Migration 155 preflights malformed legacy weekday sets, snapshots exact identity
and relationship evidence, backfills, reconciles and renames the annual preset in
one transaction. It preserves timestamps, revisions, receipts, foreign keys and
Attendance facts. The delegated writer performs no production action. The Coordinator retains
authorized Git/PR/integration and selected deployment acceptance under `.rules`.

Backend owns Rust DTOs/services/HTTP, migration, generated OpenAPI/TypeScript and
focused backend tests. Frontend owns other School frontend files. The Senior owns
Git and integration; one serialized Cargo/generator owner shares the existing cache.
