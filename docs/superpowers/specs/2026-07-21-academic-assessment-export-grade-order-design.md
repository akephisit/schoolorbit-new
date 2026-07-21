# Academic Assessment Export Grade Ordering Design

## Goal

Make both XLSX downloads on the academic assessment page list secondary grade levels in ascending order, starting with `ม.1` and continuing through `ม.6`.

## Current Behavior

The frontend export helper sorts assessment plans by subject group before grade level. This separates rows from the same grade across subject-group sections even though the assessment API already provides `gradeLevelSort` and `gradeYear` fields.

## Design

Keep XLSX generation in the existing assessment page and change only the export comparator priority:

1. grade-level type (`gradeLevelSort`)
2. grade year (`gradeYear`)
3. subject-group display order
4. subject-group name
5. classroom room number
6. subject code
7. subject title

The existing `sortedAssessmentExportPlans` helper remains the single ordering path for both the overview and exam-format downloads. Sorting must continue to operate on a copied array so the on-screen assessment rows are not mutated.

## Stack Impact

- Frontend only: `frontend-school/src/routes/(app)/staff/academic/assessments/+page.svelte`
- Frontend static regression test: `frontend-school/tests/static/academic-assessment-structure.test.mjs`
- No backend, API-contract, database, migration, permission, or deployment changes

## Error Handling

The existing download error handling and toast messages remain unchanged. This change only affects deterministic row ordering before the workbook is created.

## Verification

- Add a regression assertion proving grade sort keys precede subject-group sort keys in the export comparator.
- Run the focused academic assessment static test.
- Run Svelte autofix analysis and the frontend type checker.
- Run `git diff --check` and inspect repository status.
