# Curriculum Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add top-right XLSX exports for planned curriculum and actually assigned classroom curriculum.

**Architecture:** Keep the implementation frontend-only by reusing existing API calls and the existing `xlsx` dependency. Add small pure helpers for row construction and filtering, then wire them into page-level dialogs.

**Tech Stack:** SvelteKit 5, TypeScript, existing API client, `xlsx`, shadcn-style local UI components.

---

### Task 1: Shared Curriculum Export Helpers

**Files:**
- Create: `frontend-school/src/lib/utils/curriculum-export.ts`
- Create: `frontend-school/src/lib/utils/curriculum-export.test.ts`

- [ ] Write tests for effective-year filtering and row helpers.
- [ ] Run `npm run check` or targeted test command to verify initial type failures.
- [ ] Implement filtering and row helpers.
- [ ] Re-run checks.

### Task 2: Study Plans Export UI

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/study-plans/+page.svelte`

- [ ] Add top-right `ส่งออกหลักสูตร` outline button.
- [ ] Add export dialog with academic-year select.
- [ ] Load effective versions, subjects, and activities.
- [ ] Generate XLSX sheets `หลักสูตร` and `สรุป`.

### Task 3: Planning Export UI

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/planning/+page.svelte`

- [ ] Add top-right `ส่งออกใช้จริง` outline button.
- [ ] Add export dialog with academic-year select.
- [ ] Load every semester, every classroom, semester courses, and classroom activities.
- [ ] Generate XLSX sheets `ใช้จริง` and `สรุปห้องเรียน`.

### Task 4: Verification

**Files:**
- Verify frontend project.

- [ ] Run `cd frontend-school && npm run check`.
- [ ] Run `git diff --check`.
- [ ] Inspect `git status --short`.
