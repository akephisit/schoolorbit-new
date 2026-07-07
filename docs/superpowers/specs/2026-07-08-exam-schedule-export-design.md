# Exam Schedule Export Design

## Goal

Add a full XLSX export for one exam schedule round from the staff exam schedule detail page. The export should give academic staff a single editable workbook that also has a print-friendly report sheet, covering scheduled exams, room assignments, invigilator assignments, invigilator workload, and readiness status.

## UI Design

The staff exam schedule detail page adds a compact `ส่งออก` action in the top action area near the existing status, exam-kind selector, refresh, and publish controls.

The button downloads immediately. It does not need a dialog in the first version because the user selected the full workbook export. The workbook is one Excel file. The first sheet is a print-friendly report, and the following sheets contain editable detailed data. The exported filename uses:

`ตารางสอบ-{ชื่อรอบสอบ}-{วันที่ส่งออก}.xlsx`

The export action is available for both draft and published rounds because draft data is useful for internal checking. The button is disabled while the workbook is being generated and shows a loading state.

## Data Scope

The workbook is generated client-side from the already loaded exam schedule workspace:

- `workspace.round`
- `workspace.days`
- `workspace.scheduledSessions`
- `workspace.unscheduledItems`
- `workspace.readiness`
- day room assignments embedded in each exam day

The export also needs invigilator data from `ExamInvigilatorWorkspace`. If that workspace has not been loaded yet, clicking export loads it once before building the workbook. If invigilator loading fails, the export stops and shows the same user-facing error style as other exam schedule actions.

The export does not include sensitive staff/student PII such as national IDs, phone numbers, email addresses, addresses, or usernames.

## Workbook Format

The XLSX file contains these sheets:

- `รายงาน`: the first sheet, intended for printing.
  - Content: report title, round name, status, date range, summary counts, readiness summary, schedule grouped by exam day, and invigilator workload summary.
- `ตารางสอบ`: one row per scheduled exam session.
  - Columns: วันสอบ, วันที่, เวลาเริ่ม, เวลาจบ, ระยะเวลา, ชั้นเรียน, กลุ่มระดับ, วิชา, รหัสวิชา, กลุ่มสาระ, ประเภทวิชา, ห้องสอบ, อาคาร/ห้อง, กรรมการ.
- `ห้องสอบ`: one row per room assignment.
  - Columns: วันสอบ, วันที่, ห้องเรียน, ห้องสอบ, อาคาร, ความจุห้อง, ความจุที่ใช้, จำนวนนักเรียน, สร้างเลขที่นั่งแล้ว, จำนวนกรรมการ.
- `กรรมการ`: one row per invigilator assignment per room.
  - Columns: วันสอบ, วันที่, ห้องเรียน, ห้องสอบ, ชื่อกรรมการ, บทบาท, เวลาสอบรวมของห้อง.
- `ภาระงานกรรมการ`: one row per invigilator workload.
  - Columns: ชื่อกรรมการ, ชั่วโมงรวม, นาทีรวม, จำนวนวัน, จำนวนห้อง, รายละเอียดรายวัน.
- `ความพร้อม`: readiness summary and actionable issues.
  - Columns: ประเภท, รายการ, สถานะ/รายละเอียด.

If a sheet has no data, it still exists with headers and a single explanatory row where useful, so users can tell the export completed. The export does not try to produce a locked PDF-like layout; users can edit the workbook before printing.

## Architecture

Keep the first implementation frontend-only. Add a focused utility file under `frontend-school/src/lib/utils/` to build plain row arrays from typed exam schedule data. The route imports the utility and dynamically imports `xlsx` only when the user clicks export, matching existing curriculum and assessment export patterns.

The page owns UI state:

- `exportingExamSchedule`
- `handleExportExamSchedule()`

The utility owns deterministic row construction and filename sanitization. This keeps the Svelte route from accumulating spreadsheet formatting logic and makes row behavior testable with static or unit-style tests.

## Permissions

No new permission is introduced in the first version. The detail page already requires exam schedule read access, and the export only includes data already visible in this workspace or loaded through the invigilator workspace endpoint that is already permission-guarded.

The export button should appear to users who can read the exam schedule detail page. If the invigilator workspace endpoint rejects access, the export reports the failure and does not produce a partial workbook.

## Error Handling

Export failures show `toast.error(...)`. Success shows `toast.success(...)`.

The export should stop when:

- the main workspace is not loaded
- invigilator workspace loading fails
- dynamic `xlsx` import or workbook writing fails

The UI clears the loading state in `finally`.

## Testing

Add static tests covering:

- the exam schedule detail route exposes an export action and dynamic `xlsx` import
- the export utility exists and defines row builders for the print report sheet and all detailed sheets
- the route loads invigilator workspace before export when needed
- the route avoids exporting PII-oriented fields

Run:

- `node --test tests/static/academic-exam-schedule.test.mjs`
- `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check`
