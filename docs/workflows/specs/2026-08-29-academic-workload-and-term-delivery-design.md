# Academic Workload and Term Delivery Design

## Goal

Separate official curriculum workload from the number of timetable periods that a school chooses
to allocate in one academic term. Repair incomplete legacy catalog metrics with the school's
approved 20-week rule without creating a one-off user workflow, and prevent newly published
catalog versions from becoming incomplete again.

This design prepares a stable data boundary for a future timetable redesign. It does not implement
the future drag-and-drop timetable, timetable completion rules, automatic scheduling, or schedule
pattern constraints.

## Confirmed Business Rules

- A subject catalog version owns the official values shown in the curriculum:
  - credit;
  - standard periods per week;
  - official total hours per term.
- Official credit and official total hours do not change merely because a school allocates more
  timetable periods in one term.
- A course offering is scoped to one academic term and owns the weekly period target that the
  school intends to allocate in that term.
- A new course offering starts from the selected subject version's standard periods per week.
- Academic staff may change the course offering's weekly period target for that term. For example,
  a 0.5-credit subject with a standard value of 1 period per week may be allocated 2 periods per
  week while remaining a 0.5-credit subject with 20 official hours.
- Every learning group under the same course offering uses the same weekly period target. Learning
  groups do not have individual overrides.
- A later term or academic year starts again from the catalog standard. It must not copy the prior
  offering's override automatically.
- The approved legacy repair rule is exactly 20 instructional weeks. It is a one-time data repair,
  not a permanent formula for new data.

## Data Ownership

### Catalog version: official workload

`subject_versions` remains the source of truth for official course workload:

- `credit`: official credit used by curriculum, grading, results, and academic documents;
- `periods_per_week`: standard weekly timetable-period value and the default for a new offering;
- `hours_per_semester`: official total hours displayed in the curriculum.

The catalog UI must expose all three values when creating a subject version. A subject version may
remain incomplete while it is a draft, but publishing must fail unless all three values are present
and positive.

`activity_versions` continues to own `hours_per_week` and `hours_per_term`. Its existing publish
guard requiring total hours remains authoritative. Translating activity hours into timetable
periods is deferred until the timetable design covers non-integer weekly hours and bell-period
duration explicitly.

### Course offering: term-specific delivery target

`course_offering_details` gains a positive integer `weekly_period_target`.

- Creating a course offering copies `subject_versions.periods_per_week` into this field.
- Curriculum preparation and batch offering creation use the same default.
- Academic staff can change the value while managing the term's offering.
- The value belongs to the offering, so all groups inherit it and no group-level copy is stored.
- The value is a timetable allocation target only. It never changes credit, official total hours,
  assessment weight, or results.

Course offering read models expose both values with unambiguous names:

- `standardPeriodsPerWeek`: the catalog value;
- `weeklyPeriodTarget`: the term-specific delivery value.

The delivery UI displays both when they differ, for example:

> ตามหลักสูตร 1 คาบ/สัปดาห์ · จัดจริงภาคเรียนนี้ 2 คาบ/สัปดาห์

The current timetable API and UI do not consume or enforce the target in this release. The field is
the future timetable's authoritative input, avoiding another data-model change when drag-and-drop
scheduling is designed later.

## Legacy 20-Week Repair

Add `051_academic_workload_and_term_delivery.sql` as the next forward-only tenant migration. Never
edit migrations 041–050.

The migration repairs only missing values and never overwrites an existing value:

1. Add `course_offering_details.weekly_period_target` as nullable initially, with a positive-integer
   check constraint.
2. Temporarily disable the published-version immutability triggers on `subject_versions` and
   `activity_versions`, plus `course_offering_details_published_immutable`, inside the migration
   transaction. The offering trigger must be disabled because the one-time backfill also updates
   already-published offerings.
3. For legacy subject versions whose `migration_provenance` identifies the Academic Core migration
   and whose `periods_per_week` is null:
   - require a positive `hours_per_semester`;
   - require `hours_per_semester` to be exactly divisible by 20 because the destination is an
     integer number of periods;
   - set `periods_per_week = hours_per_semester / 20`.
4. For legacy activity versions whose `migration_provenance` identifies the Academic Core migration
   and whose `hours_per_term` is null, set `hours_per_term = hours_per_week * 20`.
5. Merge repair metadata into each repaired catalog version's `migration_provenance` without
   deleting the original provenance.
   Record the repair migration and `instructionalWeeks: 20`.
6. Increment `row_version` and update `updated_at` for every repaired version.
7. Backfill every existing course offering's `weekly_period_target` from its subject version after
   the subject repair. Merge migration 051 provenance into the offering detail without deleting its
   original provenance.
8. Fail and roll back the whole migration if any curriculum-referenced legacy subject still lacks
   complete positive official metrics, if a required subject total is not divisible by 20, if any
   curriculum-referenced legacy activity still lacks complete positive official metrics, or if any
   existing course offering cannot receive a positive target from its referenced subject version.
9. Set `weekly_period_target` to `NOT NULL` after the backfill succeeds.
10. Re-enable all three immutability triggers before completing the migration and verify that each
    is enabled. Any exception rolls back both the data changes and trigger-state changes.

The migration runs through the centralized all-tenant runner while the school API is in maintenance
mode. It does not contain a tenant UUID, subdomain, or school-specific secret. Its predicates are
based on legacy provenance and missing values, so future empty tenant databases receive no repaired
rows.

## Catalog and Delivery Workflows

### Creating a subject version

The subject version form collects:

- Thai and optional English names;
- subject type and grade levels;
- credit;
- standard periods per week;
- official total hours per term;
- effective date range.

The frontend sends the existing typed `periodsPerWeek` and `hoursPerSemester` contract fields instead
of hard-coding them to null. No compatibility DTO or untyped payload is introduced.

### Publishing a subject version

The backend checks the official metrics immediately before the draft-to-published transition. The
check is server-authoritative and returns an actionable validation error. The frontend may show the
same readiness state, but hiding or disabling a button is not the enforcement boundary.

### Creating and managing a course offering

All creation paths resolve the selected subject version and copy its standard period value into
`weekly_period_target` on the server; clients cannot accidentally carry a previous term's override
into a new offering. A draft offering management surface allows an authorized user to change the
term target once at offering level. The detail and overview responses return both standard and term
values so users do not confuse an allocation override with a curriculum change.

No permission family changes are required. Existing Learning Offering manage policies remain the
authorization boundary for changing the term target.

## Validation Boundaries

- Catalog publish: official metrics must be complete and positive.
- Curriculum publish: every referenced catalog version must be published and have complete official
  metrics.
- Course offering create: the selected subject version must have a positive standard period value.
- Course offering update: the weekly period target must be a positive integer.
- Course offering publish: the weekly period target must be present and valid.
- A difference between standard periods and the term target is valid by design and is not a
  curriculum blocker.
- Timetable entry counts are not validated against the target in this release.

## API and Contract Impact

The Rust DTOs and OpenAPI contract remain authoritative.

- Add `weeklyPeriodTarget` to the offering update request and course offering read models. The
  create request does not own this default; the backend resolves it from the selected subject
  version and returns it in the created offering.
- Add `standardPeriodsPerWeek` to the course offering presentation/read model without asking the
  frontend to infer it from unrelated fields.
- Regenerate the tracked OpenAPI artifact and generated TypeScript DTOs.
- Keep response envelopes and existing resource policies unchanged.

No realtime payload changes are required. A successful offering mutation may continue to use the
existing delivery refresh behavior.

## Future Timetable Boundary

A later timetable design will consume `course_offering_details.weekly_period_target` and may present
progress such as `2/3 คาบ`. That later design must separately decide:

- drag-and-drop interaction and keyboard/mobile alternatives;
- whether excess periods warn or block;
- consecutive-period patterns such as `1+1+1` or `2+1`;
- week ranges, alternating weeks, summer, and other non-weekly patterns;
- actual delivered periods versus the recurring timetable template;
- activity scheduling targets and conversion between clock hours and bell periods;
- timetable publication, reopening, approval, and audit behavior.

Those decisions must not be encoded implicitly in the catalog repair migration or this release's UI.

## Testing and Verification

Focused tests cover:

- migration repair of a 40-hour subject to 2 standard periods per week;
- migration repair of a 1.00-hour-per-week activity to 20.00 hours per term;
- no overwrite when either destination value is already present;
- exact failure and transaction rollback for a non-divisible subject total;
- preservation of migration provenance and re-enabled immutability triggers;
- subject publishing rejected when either official workload field is missing;
- subject publishing accepted when credit, periods, and total hours are complete;
- course offering creation defaults the weekly target from the subject version;
- an offering-level override affects all groups through the shared offering and does not mutate the
  catalog version;
- a new term's offering starts again from the catalog standard;
- frontend subject-version payloads include both workload fields;
- delivery UI labels distinguish standard and term-specific values.

Run the applicable API-contract, backend-school, frontend-school, migration, diff, and status gates
from `.rules`. Database tests use the disposable test runner. Neon compatibility, when run, uses only
the explicit disposable-branch gate.

## Deployment and Recovery

- Keep the protected Neon snapshot until migration and authenticated read-only verification pass.
- Deploy through the normal backend workflow so maintenance mode, all-tenant migration status, and
  readiness gates remain authoritative.
- Do not run ad-hoc SQL against the live tenant or edit SQLx migration history.
- If migration validation fails, the transaction rolls back and maintenance stays active. Correct
  the exceptional source data through a reviewed forward migration or application artifact.
- Because this migration changes published legacy catalog rows intentionally, do not deploy an older
  backend against a database that has accepted later writes after this release. Repair forward.

## Out of Scope

- cloning a published curriculum into a new draft solely to repair these metrics;
- bulk replacement of catalog versions in curricula;
- timetable drag-and-drop;
- automatic timetable generation;
- per-group weekly period overrides;
- changing official credit or total hours to match an offering override;
- deriving future catalog data automatically from a fixed 20-week rule;
- term closure, year closure, promotion, Gradebook, or academic documents.
