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

## Word export

Teachers can select questions across result pages and export them as an A4 `.docx` document. The
browser loads the full details of the selected questions before building the file, so choices and
every referenced image are included. Image bytes are read through the authenticated question-bank
file endpoint, which verifies that the user can read the question and that the file is actually
referenced by it; export does not depend on cross-origin access to the public storage URL. The export
dialog can optionally append correct choices, explanations, and scoring rubrics as an answer key.

Normal text remains editable Word text using `TH Sarabun New`, and uploaded images are converted to
PNG and embedded in the document. Formulas are not exported as images: MathLive LaTeX is converted to
MathML and then to native Office Math Markup Language (OMML). Inline formulas stay in their sentence,
display formulas remain centered, and users can click and edit both with Word's equation tools. This
uses Word's native equation model and gives the editable MathType-style result expected in modern
Word; it does not embed MathType's proprietary OLE object. The exporter and its conversion
dependencies are client-only; the SSR build resolves the module to a small server stub so they are
not bundled into the Cloudflare Worker. Standard function names such as `sin`, `cos`, `tan`, `log`,
and `ln` remain in the equation's Cambria Math font but use its upright/plain style, followed by
compact mathematical function spacing and italic variables. Compact input such as `sinx` is
normalized the same way. Exported paragraphs use single line spacing while still allowing Word to
expand a line for tall fractions or roots, and every equation run is normalized to regular (non-bold)
weight. Paragraph spacing after is zero throughout the exported document, and question numbers and
choice labels use the same regular weight as their surrounding text.

## API

- `GET /api/academic/question-bank/options`
- `GET /api/academic/question-bank/questions` (search, filters, and pagination)
- `GET /api/academic/question-bank/questions/{id}`
- `GET /api/academic/question-bank/questions/{question_id}/files/{file_id}`
- `POST /api/academic/question-bank/questions`
- `PUT /api/academic/question-bank/questions/{id}`
- `DELETE /api/academic/question-bank/questions/{id}`
