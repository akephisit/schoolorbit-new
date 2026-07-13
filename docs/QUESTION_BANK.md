# Question bank

The question bank is a reusable storage workspace. It does not model academic years, exam papers,
approval workflows, or question collections.

## Data contract

- Every new or updated question must reference one exact `subjects.id` catalog record.
- Grade level is not stored by the question-bank workflow. It follows the selected subject and the
  curriculum/course configuration around that subject.
- Existing legacy questions with no subject remain readable so authorized staff can open and repair
  them. Saving one requires selecting a subject.
- Rich content supports paragraph, math, and image blocks. Teachers write normal text in an
  article-style editor and build math visually from symbol buttons or the full math keyboard; they do
  not need to type LaTeX. LaTeX remains only as the internal storage format. The editor changes the
  first block of each supported type while preserving additional blocks it does not edit.

## Access

- Assigned scope includes every instructor in `classroom_course_instructors`, including team teachers.
- Subject-group scope follows organization-unit resource access.
- School scope can read or manage the whole bank.
- The question-bank `/options` endpoint returns exact subject records under question-bank permissions;
  the page does not depend on curriculum-management permissions.

## Images

Selecting a local image only creates a browser preview. No upload occurs until the user saves.
During save, new images are uploaded as temporary `course_material` files. The question transaction
validates ownership/type and finalizes those files atomically. Temporary uploads expire after 24 hours,
and the scheduled file cleaner removes expired objects and metadata.

## API

- `GET /api/academic/question-bank/options`
- `GET /api/academic/question-bank/questions` (search, filters, and pagination)
- `GET /api/academic/question-bank/questions/{id}`
- `POST /api/academic/question-bank/questions`
- `PUT /api/academic/question-bank/questions/{id}`
- `DELETE /api/academic/question-bank/questions/{id}`
