# Design: Supervision Rubric Template Builder

## Goal

Improve the teaching supervision evaluation template so it matches real school paper forms: multiple sections, many rubric items, a 1-5 quality scale, evaluator comments, total score, percentage, quality level, and observed-teacher acknowledgement.

The current backend domain already supports this shape through `supervision_template_sections`, `supervision_template_items`, and `supervision_evaluator_responses`. The gap is mostly UI and workflow polish: the current create-template dialog only creates a very small "basic" template with one rating item and one text item.

## Key Decisions

- Use the existing supervision template data model. Do not add a parallel "legacy/simple template" path.
- Replace the current simple template dialog with a rubric builder inside the same supervision page.
- Include a preset based on the provided paper form so schools can start from a realistic template and edit it.
- Keep the template school-wide for now, consistent with the existing supervision design.
- Keep item types simple in this round: `rating` and `text`.
- Use the existing global template rating range, defaulting to `1-5`.
- Compute total score, percentage, and quality level from rating responses instead of storing duplicate summary numbers.
- Evaluation and report screens must render section/item structure, not just a flat average.

## Preset Rubric

The default preset should create these sections and rating items. Labels can be edited before saving.

### 1. ลักษณะการปฏิบัติงาน

- 1.1 การตรงต่อเวลา
- 1.2 การควบคุมความเป็นระเบียบในชั้นเรียน
- 1.3 การรักษาความสะอาดในชั้นเรียน

### 2. บุคลิกภาพ

- 2.1 การแต่งกายสุภาพ เหมาะสม
- 2.2 การใช้น้ำเสียงมีความชัดเจน
- 2.3 ความเชื่อมั่นในตนเอง
- 2.4 การใช้ภาษาสื่อสารและสร้างบรรยากาศการเรียนรู้

### 3. การจัดกิจกรรมการเรียนรู้

- 3.1 แจ้งจุดประสงค์การเรียนรู้รายชั่วโมง
- 3.2 เนื้อหาสอดคล้องกับจุดประสงค์การเรียนรู้/ผลการเรียนรู้
- 3.3 จัดกิจกรรมการเรียนรู้อย่างเป็นลำดับขั้นตอนตามแผนการจัดการเรียนรู้
- 3.4 มีกิจกรรมการเรียนรู้ด้วยวิธีการที่หลากหลาย
- 3.5 มีการตั้งคำถามที่กระตุ้นให้ผู้เรียนใช้กระบวนการคิด และร่วมแสดงความคิดเห็น
- 3.6 ใช้สื่อที่สอดคล้องและเหมาะสมกับสาระการเรียนรู้
- 3.7 มีการสอดแทรกคุณลักษณะอันพึงประสงค์ คุณธรรมจริยธรรม และความรู้ทั่วไป
- 3.8 จัดบรรยากาศการเรียนรู้ที่ดึงดูดความสนใจ ก่อให้เกิดความกระตือรือร้น
- 3.9 มีการให้การเสริมแรงเชิงบวกในชั้นเรียน
- 3.10 มีการสรุปเนื้อหาได้ตรงตามจุดประสงค์การเรียนรู้
- 3.11 การชี้แนะการเรียนรู้/การศึกษาค้นคว้า และแหล่งเรียนรู้ค้นคว้าเพิ่มเติม

### 4. การวัดและประเมินผล

- 4.1 สอดคล้องและครอบคลุมจุดประสงค์การเรียนรู้
- 4.2 การประเมินหลากหลายวิธี

### Text Items

Add one required or optional text item at the end:

- ความคิดเห็นและข้อเสนอแนะ

## Template Builder UX

The template tab should become a real builder:

- Show existing templates as before, but add an edit action for users with `supervision.manage.school`.
- Create/edit template opens a larger dialog or page-style panel using local shadcn-svelte components.
- Builder fields:
  - Template title
  - Description
  - Status
  - Rating min/max
  - Sections
  - Items inside each section
- Users can:
  - Load the paper-form preset
  - Add section
  - Rename section
  - Remove section
  - Add rating item
  - Add text item
  - Edit item label and description
  - Toggle required
  - Remove item
  - Reorder sections/items with simple up/down controls first; drag-and-drop can come later
- Save uses the existing `createSupervisionTemplate` and `updateSupervisionTemplate` API payloads with nested `sections`.

## Evaluation UX

When an evaluator opens an assigned observation:

- Render sections in order.
- Render rating items as a compact 5-level selector, not a raw number input.
- Render text items as textarea.
- Show per-section progress such as "8/11 answered".
- Show draft state clearly.
- On submit, validate required items client-side before sending.
- Backend remains the authority and should still validate rating range.

## Report / Review UX

For report and review screens:

- Keep the observation list compact.
- When a reviewer selects an observation, show a detail panel or dialog:
  - Observed teacher
  - Lesson snapshot
  - Evaluators and submission status
  - Section-by-section scores
  - Total score
  - Percentage
  - Quality level
  - Text comments
  - Acknowledgement status/comment
- Quality level should follow the paper form:
  - `>= 90%`: ดีมาก
  - `80-89%`: ดี
  - `70-79%`: พอใช้
  - `60-69%`: ควรปรับปรุง
  - `< 60%`: ไม่ผ่าน

## Data Flow

No new database tables are required for the builder itself.

1. The frontend loads templates through `listSupervisionTemplates()`.
2. The builder edits a local `CreateSupervisionTemplateRequest` / `UpdateSupervisionTemplateRequest`.
3. On save, the backend replaces the section/item set using existing service behavior.
4. Evaluators save responses through existing `saveMySupervisionEvaluation()` / `submitMySupervisionEvaluation()`.
5. Scores are derived from `SupervisionTemplate.sections[].items` plus saved evaluator responses.

If the frontend cannot currently load saved evaluator responses for display, add typed response data to the observation detail API rather than creating a separate ad-hoc endpoint.

## Permissions

- Viewing the supervision workspace remains guarded by the supervision module permission/read/request permissions.
- Creating/editing templates requires `supervision.manage.school`.
- Evaluating assigned observations requires `supervision.evaluate.assigned`.
- Reviewing/publishing results remains under `supervision.approve.school`.
- Frontend buttons and tabs should follow the existing `can` store pattern; backend policies remain authoritative.

## Testing

Add static and behavior checks:

- Static contract: supervision page must not create only `ratingLabel`/`textLabel` basic templates.
- Static contract: template creation must send nested `sections` and `items`.
- Frontend check: `npm run test:static`, `npm run check`, `npm run lint`.
- Backend check if response DTOs change: `cargo check` and focused service tests.
- If adding score helper functions, add unit tests for total score, percentage, and quality-level thresholds.

## Out Of Scope

- File/image attachment upload for scanned paper forms.
- Digital signature rendering.
- Per-subject-group custom templates.
- Drag-and-drop ordering for sections/items.
- PDF export.

These can be added later after the rubric builder and evaluation detail flow are stable.
