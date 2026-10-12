# Attendance direct results and note removal

Approved in chat on 2026-10-12: replace per-person dropdowns with five visible single-selection status buttons and remove per-person notes from UI, API, database and historical result audit snapshots. The user accepted permanent removal of old note text and requested implementation.

The existing Attendance domain owns result validation and persistence. Keep sparse results, immutable rosters, generation/tokens, correction reasons, school/assigned/own permissions and post-commit notification behavior. Calendar-day notes are outside this scope. No module reset or unrelated data removal.

Use the installed Bits UI Toggle Group through shared School UI wrappers. Select one status per pupil with a single tap; tapping the selected status retains it. Explicit reset is available only for pupils whose baseline is unchecked. Mobile places the name above the control and keeps at least 44-pixel status targets; desktop uses an inline table. Retain whole-round save and correction/conflict handling.

Remove note from result/history/input DTOs and regenerate OpenAPI/TypeScript. New migration 106 validates historical Attendance result audit tuples, stages their deterministic conversion to named result snapshots without notes, reconciles unchanged audit identity/actor/time/reason and non-note record facts, then removes the record column. Unknown audit shapes fail with a bounded finding code before cleanup. Existing audit entries remain, including entries whose sessions were reset or archived.

The new runtime can read schema 105 while the old note column is still present, but the previous runtime cannot use schema 106. Deploy both consumers with the coordinated maintenance/all-tenant migration gates. Once any tenant applies 106, recover only with a compatible forward fix; never restore the prior Attendance binary or applied migration. The feature-level schema removal applies to all deployed School tenants.

Verify preservation, ambiguous-shape refusal and safe retry on owned PostgreSQL; sparse first saves, corrections, stale roster/revision refusal and old-field rejection; generated contracts; keyboard, read-only, reset, mobile/desktop light/dark and conflict browser flows. Complete the auto verification matrix and deployed acceptance before claiming delivery. Execute inline under the repository rules.
