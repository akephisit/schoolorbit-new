# Academic Assessment Structure Plan

## Goal

Build a score-structure setup tied to opened classroom courses, so teachers can declare whether each score bucket is an in-timetable exam, outside-timetable exam, or non-exam work, and can later split each bucket into smaller score items.

## Scope

- Add course-level assessment plans for existing `classroom_courses`.
- Add top-level categories such as before midterm, midterm, after midterm, and final.
- Allow each category to carry `max_score`, `exam_mode`, display order, and child score items.
- Do not force a course total of 100.
- Validate that names are present and scores are non-negative.
- Surface plan status for academic overview: not configured, draft, submitted, locked.
- Add downloadable overview data from the frontend from the same API payload.

## Backend Steps

1. Add migration `012_academic_assessment_plans.sql`.
   - `academic_assessment_plans`
   - `academic_assessment_categories`
   - `academic_assessment_items`
   - checks for status, category code, exam mode, and non-negative scores.
2. Add permissions.
   - `academic_assessment.read.assigned`
   - `academic_assessment.manage.assigned`
   - `academic_assessment.read.school`
   - `academic_assessment.manage.school`
3. Add models in `backend-school/src/modules/academic/models/assessment.rs`.
4. Add service in `backend-school/src/modules/academic/services/assessment_service.rs`.
   - `default_categories()`
   - `allocation_status(max_score, item_total)`
   - `validate_plan_payload(...)`
   - `list_assessment_plans(...)`
   - `get_or_create_plan_detail(...)`
   - `save_plan(...)`
   - `submit_plan(...)`
   - authorization helpers for assigned instructors.
5. Add thin handlers in `backend-school/src/modules/academic/handlers/assessment.rs`.
6. Register routes under `/api/academic/assessments`.

## API Shape

```http
GET /api/academic/assessments/plans?academic_semester_id=...
GET /api/academic/assessments/courses/{course_id}
PUT /api/academic/assessments/courses/{course_id}
POST /api/academic/assessments/courses/{course_id}/submit
```

`PUT` payload:

```json
{
  "categories": [
    {
      "id": "optional-uuid",
      "code": "midterm",
      "name": "กลางภาค",
      "maxScore": 30,
      "examMode": "in_timetable",
      "displayOrder": 20,
      "items": [
        {
          "id": "optional-uuid",
          "name": "ข้อสอบกลางภาค",
          "maxScore": 20,
          "displayOrder": 10,
          "isActive": true
        }
      ]
    }
  ]
}
```

## Frontend Steps

1. Add permission constants in `frontend-school/src/lib/permissions/registry.ts`.
2. Add API client in `frontend-school/src/lib/api/academicAssessments.ts`.
3. Add route metadata and page at `frontend-school/src/routes/(app)/staff/academic/assessments/`.
4. Page layout:
   - header action: download dropdown at the top-right, matching current academic export pattern.
   - filters: semester, classroom, instructor, status.
   - overview cards: total courses, draft/submitted/locked, outside-timetable exams.
   - table: course, class, teacher, total score, category count, exam mode summary, status.
   - editor dialog: categories and nested score items, with add/remove controls.

## Tests And Verification

1. Backend unit tests for score allocation and payload validation before production code.
2. Frontend static test for route metadata, permission registry, and API client contract.
3. Run focused checks:
   - `cargo test assessment_service`
   - `npm run test:static`
   - `npm run check`
   - `git diff --check`
