# Question bank

The question bank is a reusable storage workspace. It does not model academic years, exam papers,
approval workflows, or question collections.

## Data contract

- Every new or updated question must reference one exact `subjects.id` catalog record.
- Grade level is not stored by the question-bank workflow. It follows the selected subject and the
  curriculum/course configuration around that subject.
- Existing legacy questions with no subject remain readable so authorized staff can open and repair
  them. Saving one requires selecting a subject.
- Rich content is stored as versioned structured JSON (`schemaVersion: 1`), not HTML. The document
  supports paragraphs containing normal text and inline math, separate math blocks, and draggable
  image blocks. LaTeX is only the internal value of a math node; teachers insert and edit math through
  MathLive symbol controls in the same editor as normal text.
- The API and database accept only the typed node set. Read-only rendering walks that tree and renders
  text directly, math through KaTeX with untrusted commands disabled, and images through validated
  file records. Editor-only `blob:` preview URLs and pending IDs are removed before an API request.
- `search_text` is the application-maintained plain-text/LaTeX projection of the stem document. This
  keeps search useful without querying serialized JSON or storing rendered HTML.

Example persisted content:

```json
{
  "schemaVersion": 1,
  "document": {
    "type": "doc",
    "content": [
      {
        "type": "paragraph",
        "content": [
          { "type": "text", "text": "จากสมการ " },
          { "type": "inline_math", "attrs": { "latex": "x=1-2x" } },
          { "type": "text", "text": " ค่า x เท่ากับเท่าใด" }
        ]
      },
      {
        "type": "image",
        "attrs": {
          "fileId": "00000000-0000-0000-0000-000000000000",
          "altText": "กราฟประกอบโจทย์",
          "caption": null,
          "alignment": "center",
          "widthPercent": 60
        }
      }
    ]
  }
}
```

## Access

- Assigned scope includes every instructor in `classroom_course_instructors`, including team teachers.
- Subject-group scope follows organization-unit resource access.
- School scope can read or manage the whole bank.
- The question-bank `/options` endpoint returns exact subject records under question-bank permissions;
  the page does not depend on curriculum-management permissions.

## Images

Selecting, pasting, or dropping a local image only creates a browser preview. Multiple image blocks can
be dragged before, after, or between paragraphs. No upload occurs until the user saves. During save,
new images are uploaded as temporary `course_material` files, pending IDs are replaced with real
`fileId` values, and the question transaction validates ownership/type and finalizes those files
atomically. Temporary uploads expire after 24 hours, and the scheduled file cleaner removes expired
objects and metadata.

## API

- `GET /api/academic/question-bank/options`
- `GET /api/academic/question-bank/questions` (search, filters, and pagination)
- `GET /api/academic/question-bank/questions/{id}`
- `POST /api/academic/question-bank/questions`
- `PUT /api/academic/question-bank/questions/{id}`
- `DELETE /api/academic/question-bank/questions/{id}`
