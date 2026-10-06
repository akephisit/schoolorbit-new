# Curriculum revisions selected by the school

## Approved behavior

The school selects a published curriculum revision and study program when arranging a homeroom. A student entering M.1 in 2572 may use the revision amended in 2569. The revision year identifies an edition; it never limits the academic years in which that edition can be selected. Academic years continue to own homerooms, enrollment and delivery.

Display `หลักสูตรสถานศึกษา → ฉบับปรับปรุง พุทธศักราช 2569 → ระดับมัธยมศึกษาตอนต้น/ตอนปลาย → แผนการเรียน → ชั้น → ภาคเรียน → รายวิชาและกิจกรรม`. A common revision heading groups presentation; the existing curriculum owner remains separate by level. Program selectors show the edition and program together and include only published programs with requirements for the selected grade.

## Canonical data and preservation

Replace curriculum-version start/end academic-year references with an optional `revision_year`. Existing explicit numeric edition names provide the backfill year; editions without an explicit revision year retain an unknown year. Preserve removed legacy year references in migration provenance. New edition requests require a Buddhist revision year, without academic-year lookups. New draft editions start with two editable regular term slots; clones retain the source slots and complete program structure. Published editions remain immutable.

Remove calendar applicability checks consistently from program options, homeroom/student-year validation, promotion references, activation references and curriculum-driven offering creation. Preserve publication, active-curriculum, permission, grade and year-lifecycle checks. Edition selection remains explicit; publishing a newer edition never rewrites room or student assignments.

The identified migrated hierarchy contains three lower-secondary and two upper-secondary curricula whose programs have placeholder names. Repair only that verified legacy shape: consolidate by level and matching revision; retain all program and requirement IDs and external program references, and record the original curriculum/version/slot mapping in provenance. Reject partial or ambiguous input. Do not change credits, course/activity content or invent second-term requirements.

## Verification and release

Use sanitized legacy fixtures for migration success, preservation, safe retry and rejection cases. Verify a published 2569 revision can be selected in 2572 and that draft/inactive/wrong-grade programs remain refused. Run backend focused tests, architecture checks, workspace checks, OpenAPI generation/checks, frontend lint/check/static checks and fixture-backed desktop/mobile light/dark browser flows.

Schema and matched consumers release together under maintenance through the central tenant runner. Older binaries using removed year columns are prohibited after cutover. Retain the protected recovery snapshot required by `docs/OPERATIONS.md` before production data replacement. Do not apply schema files manually or bypass failed release checks.
