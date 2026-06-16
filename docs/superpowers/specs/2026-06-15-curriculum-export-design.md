# Curriculum Export Design

## Goal

Add two XLSX exports for academic curriculum review:

- `study-plans` export: curriculum versions that are effective in a selected academic year.
- `planning` export: courses and learner-development activities actually assigned to classrooms in a selected academic year.

## UI Design

Both pages use a small outline action in the top-right page header. The button opens a compact dialog with an academic-year selector and a download action.

- `/staff/academic/study-plans`: button label `ส่งออกหลักสูตร`.
- `/staff/academic/planning`: button label `ส่งออกใช้จริง`.

The selected year defaults to the current academic year when available, otherwise the first available year.

## Data Scope

Study-plan export includes every active study-plan version whose effective range covers the selected year:

- `start_academic_year_id` year is less than or equal to selected year.
- `end_academic_year_id` is empty or its year is greater than or equal to selected year.

For each included version, the file includes study-plan subjects and plan activities.

Planning export includes all classroom data actually assigned in the selected year:

- Every classroom in the selected academic year.
- Every semester in the selected academic year.
- Every `classroom_courses` row returned for those semesters.
- Every activity slot assigned to each classroom/semester.

## File Format

Both exports are XLSX files generated client-side using the existing `xlsx` dependency.

Study-plan export sheets:

- `หลักสูตร`: one row per subject or activity in an effective study-plan version.
- `สรุป`: one row per effective study-plan version.

Planning export sheets:

- `ใช้จริง`: one row per actual course or actual classroom activity.
- `รายวิชา-กิจกรรม`: one row per subject code per term and one row per activity name/type per term. It aggregates the classrooms that use each item and lists all instructors for classroom courses.
- `สรุปห้องเรียน`: counts per classroom and term.

## Permissions

No new permissions are introduced. The pages already require:

- `academic_curriculum.read.all` for study plans.
- `academic_course_plan.read.all` for planning.

The exports use the same API calls already available on each page, so backend authorization remains centralized in existing endpoints.

## Error Handling

The UI disables the download button while exporting. Failures show `toast.error(...)`; successful exports show `toast.success(...)`.
