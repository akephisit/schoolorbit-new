# Exam Schedule Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a one-click XLSX export for exam schedule detail pages, with a print-friendly `รายงาน` sheet plus detailed editable sheets.

**Architecture:** Keep export generation frontend-only by adding `frontend-school/src/lib/utils/exam-schedule-export.ts` for deterministic row construction and filename sanitization. Wire a `ส่งออก` LoadingButton into the existing staff exam schedule detail page, dynamically import `xlsx`, load invigilator workspace if missing, and write one workbook.

**Tech Stack:** SvelteKit 5, TypeScript, local shadcn-svelte/LoadingButton components, existing `xlsx` dependency, static Node tests.

---

### Task 1: Static Export Contract Tests

**Files:**
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`

- [ ] Add a failing static test named `exam schedule detail exports one editable report workbook`.
- [ ] Assert the route imports `buildExamScheduleExportWorkbook`, `examScheduleExportFileName`, uses `exportingExamSchedule`, defines `handleExportExamSchedule`, dynamically imports `xlsx`, appends sheets named `รายงาน`, `ตารางสอบ`, `ห้องสอบ`, `กรรมการ`, `ภาระงานกรรมการ`, and `ความพร้อม`, and shows a `ส่งออก` action.
- [ ] Assert the route does not mention PII-oriented fields such as `nationalId`, `phone`, `email`, or `username` in the export handler.
- [ ] Run `node --test tests/static/academic-exam-schedule.test.mjs`; expected result is FAIL because the export utility and route wiring do not exist yet.

### Task 2: Export Utility

**Files:**
- Create: `frontend-school/src/lib/utils/exam-schedule-export.ts`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`

- [ ] Add typed helpers:
  - `examScheduleExportFileName(roundName: string, exportedAt = new Date())`
  - `buildExamScheduleExportWorkbook(workspace, invigilatorWorkspace)`
  - `reportRows(...)`
  - `scheduleRows(...)`
  - `roomRows(...)`
  - `invigilatorRows(...)`
  - `workloadRows(...)`
  - `readinessRows(...)`
- [ ] Utility output should be plain arrays/objects that the route can pass to `XLSX.utils.aoa_to_sheet` or `XLSX.utils.json_to_sheet`.
- [ ] Keep data PII-safe by using display names, classroom/room labels, subject labels, schedule dates/times, capacity counts, and readiness text only.
- [ ] Run `node --test tests/static/academic-exam-schedule.test.mjs`; expected result is still FAIL until route wiring is added.

### Task 3: Route Export Wiring

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte`

- [ ] Import `Download` from `lucide-svelte`.
- [ ] Import `buildExamScheduleExportWorkbook` and `examScheduleExportFileName`.
- [ ] Add `let exportingExamSchedule = $state(false);`.
- [ ] Add `ensureInvigilatorWorkspaceForExport(roundId: string)` to reuse existing state or load `getExamInvigilatorWorkspace(roundId)`.
- [ ] Add `handleExportExamSchedule()` that:
  - returns early if `workspace` is null
  - sets `exportingExamSchedule = true`
  - loads invigilator workspace if needed
  - imports `xlsx`
  - calls `buildExamScheduleExportWorkbook(workspace, invigilatorData)`
  - appends sheets in this order: `รายงาน`, `ตารางสอบ`, `ห้องสอบ`, `กรรมการ`, `ภาระงานกรรมการ`, `ความพร้อม`
  - writes `examScheduleExportFileName(workspace.round.name)`
  - shows success/error toast
  - clears loading in `finally`
- [ ] Add a compact `ส่งออก` LoadingButton in the page actions.
- [ ] Run `node --test tests/static/academic-exam-schedule.test.mjs`; expected result is PASS.

### Task 4: Verification, Commit, Push

**Files:**
- All changed files from Tasks 1-3.

- [ ] Run `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check` in `frontend-school`; expected result is 0 errors and 0 warnings.
- [ ] Run `git diff --check`; expected result is no output.
- [ ] Commit with `feat: export exam schedule report workbook`.
- [ ] Push `main` to `origin/main`.
