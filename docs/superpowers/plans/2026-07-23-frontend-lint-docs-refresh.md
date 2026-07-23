# Frontend Lint and Documentation Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the frontend lint command pass without rule suppression and bring current project guidance to the 178-operation API checkpoint.

**Architecture:** Treat the existing ESLint and Prettier failures as the red regression gate. Preserve current component behavior while replacing non-reactive mutable collections and unsafe `finally` control flow, then update only living guidance and annotate the dated improvement analysis instead of rewriting historical plans.

**Tech Stack:** SvelteKit 5, TypeScript, ESLint, Prettier, Svelte MCP autofixer, Markdown, Git

## Global Constraints

- No new dependency, route, API, database migration, or user-visible feature.
- Do not disable or weaken lint rules.
- Historical implementation plans and specifications remain unchanged.
- The generated OpenAPI artifact remains the source of truth for the 178-operation count.
- Use official Svelte 5 `SvelteMap`, `SvelteSet`, and writable `$derived` semantics.

---

### Task 1: Establish the failing quality baseline

**Files:**
- Test: `frontend-school/` through existing package scripts

**Interfaces:**
- Consumes: current `package.json`, ESLint configuration, and Prettier configuration.
- Produces: recorded red gates of 11 ESLint errors and 26 Prettier files.

- [ ] **Step 1: Install the locked frontend dependencies**

Run:

```bash
cd frontend-school && npm ci
```

Expected: exit 0 with dependencies installed from `package-lock.json`.

- [ ] **Step 2: Verify the ESLint red gate**

Run:

```bash
cd frontend-school && npx eslint .
```

Expected: exit 1 with exactly 11 errors covering writable derived state,
reactive `Map`/`Set`, unsafe `finally`, and the unused dependency expression.

- [ ] **Step 3: Verify the Prettier red gate**

Run:

```bash
cd frontend-school && npx prettier --check .
```

Expected: exit 1 with 26 files reported.

### Task 2: Correct Svelte reactive state

**Files:**
- Modify: `frontend-school/src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte`
- Modify: `frontend-school/src/lib/components/academic/exam-schedule/ExamItemTray.svelte`
- Modify: `frontend-school/src/lib/components/academic/exam-schedule/PersonalExamScheduleView.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte`
- Test: official Svelte autofixer and ESLint

**Interfaces:**
- Consumes: `SvelteMap` and `SvelteSet` from `svelte/reactivity`.
- Produces: the same arrays and option lists with mutations visible to Svelte's dependency tracking.

- [ ] **Step 1: Replace synchronized workspace state with writable derived state**

In `ExamInvigilatorPanel.svelte`, replace:

```ts
let localWorkspace = $state<ExamInvigilatorWorkspace | null>(null);
```

with:

```ts
let localWorkspace = $derived(workspace);
```

Delete the effect:

```ts
$effect(() => {
	localWorkspace = workspace;
});
```

The existing `applyWorkspace()` assignment remains valid because Svelte 5
derived values are writable.

- [ ] **Step 2: Replace mutable native maps in exam components**

Add:

```ts
import { SvelteMap } from 'svelte/reactivity';
```

to `ExamInvigilatorPanel.svelte`, `ExamItemTray.svelte`, and
`PersonalExamScheduleView.svelte`. Replace every lint-reported
`new Map<...>()` in those files with `new SvelteMap<...>()`.

- [ ] **Step 3: Replace mutable native collections in the daily timetable**

Add:

```ts
import { SvelteMap, SvelteSet } from 'svelte/reactivity';
```

to `staff/academic/timetable/today/+page.svelte`, then replace:

```ts
new Set<string>()
new Map<string, string>()
```

with:

```ts
new SvelteSet<string>()
new SvelteMap<string, string>()
```

The existing `toSortedOptions(values: Set<string> | Map<string, string>)`
signature remains valid because both reactive classes extend their native
counterparts.

- [ ] **Step 4: Verify the focused reactivity fixes**

Run the official Svelte autofixer on all four files, then run:

```bash
cd frontend-school && npx eslint \
  src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte \
  src/lib/components/academic/exam-schedule/ExamItemTray.svelte \
  src/lib/components/academic/exam-schedule/PersonalExamScheduleView.svelte \
  'src/routes/(app)/staff/academic/timetable/today/+page.svelte'
```

Expected: exit 0 with no errors or warnings.

- [ ] **Step 5: Commit the reactivity fixes**

```bash
git add frontend-school/src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte \
  frontend-school/src/lib/components/academic/exam-schedule/ExamItemTray.svelte \
  frontend-school/src/lib/components/academic/exam-schedule/PersonalExamScheduleView.svelte \
  'frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte'
git commit -m "fix(frontend): use Svelte reactive collections"
```

### Task 3: Correct stale-request cleanup flow

**Files:**
- Modify: `frontend-school/src/routes/(app)/parent/student/[id]/exams/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte`
- Test: official Svelte autofixer and ESLint

**Interfaces:**
- Consumes: existing monotonically increasing request tokens.
- Produces: stale responses remain ignored while `finally` performs cleanup only.

- [ ] **Step 1: Make the parent page dependency explicit**

Change the loader to accept the student ID it loads:

```ts
async function loadSchedules(requestedStudentId: string) {
	const requestToken = ++scheduleRequestToken;
	loading = true;
	error = '';
	rounds = [];
	try {
		const nextRounds = await listChildExamSchedules(requestedStudentId);
		if (requestToken !== scheduleRequestToken) return;
		rounds = nextRounds;
	} catch (loadError: unknown) {
		if (requestToken !== scheduleRequestToken) return;
		console.error(loadError);
		error = loadError instanceof Error ? loadError.message : 'โหลดตารางสอบของนักเรียนไม่สำเร็จ';
		toast.error(error);
	} finally {
		if (requestToken === scheduleRequestToken) {
			loading = false;
		}
	}
}

$effect(() => {
	void loadSchedules(studentId);
});
```

Change the retry callback to:

```svelte
onaction={() => loadSchedules(studentId)}
```

- [ ] **Step 2: Keep management-option cleanup conditional without returning**

Replace the `loadManagementOptions()` `finally` block with:

```ts
} finally {
	if (isCurrentManagementOptionsRequest(requestToken, roundId, semesterId, yearId)) {
		optionsLoading = false;
	}
}
```

- [ ] **Step 3: Verify the focused request-flow fixes**

Run the official Svelte autofixer on both files, then run:

```bash
cd frontend-school && npx eslint \
  'src/routes/(app)/parent/student/[id]/exams/+page.svelte' \
  'src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'
```

Expected: exit 0 with no errors or warnings.

- [ ] **Step 4: Commit the request-flow fixes**

```bash
git add \
  'frontend-school/src/routes/(app)/parent/student/[id]/exams/+page.svelte' \
  'frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'
git commit -m "fix(frontend): keep stale request cleanup safe"
```

### Task 4: Apply the existing Prettier contract

**Files:**
- Modify: the 26 files reported by `npx prettier --check .`
- Test: `npx prettier --check .`

**Interfaces:**
- Consumes: repository `.prettierrc` and `.prettierignore`.
- Produces: formatting-only changes with no new behavior.

- [ ] **Step 1: Format the failing files using the repository configuration**

Run:

```bash
cd frontend-school && npx prettier --write \
  src/lib/api/parents.ts \
  src/lib/components/academic/exam-schedule/ExamItemTray.svelte \
  src/lib/components/academic/exam-schedule/ExamRoomAssignmentPanel.svelte \
  src/lib/components/academic/exam-schedule/ExamSessionBlock.svelte \
  src/lib/components/academic/exam-schedule/invigilatorDrag.ts \
  src/lib/components/academic/exam-schedule/InvigilatorStaffList.svelte \
  src/lib/components/academic/exam-schedule/ReadinessPanel.svelte \
  src/lib/server/route-preview-meta.ts \
  src/lib/utils/examScheduleDayOrder.ts \
  'src/routes/(app)/parent/student/[id]/exams/+page.svelte' \
  'src/routes/(app)/staff/+page.svelte' \
  'src/routes/(app)/staff/academic/activities/+page.svelte' \
  'src/routes/(app)/staff/academic/assessments/+page.svelte' \
  'src/routes/(app)/staff/academic/exam-schedules/+page.svelte' \
  'src/routes/(app)/staff/academic/subject-groups/[id]/+page.svelte' \
  'src/routes/(app)/staff/academic/subject-groups/+page.svelte' \
  'src/routes/(app)/staff/academic/timetable/templates/+page.svelte' \
  'src/routes/(app)/student/exams/+page.svelte' \
  tests/static/academic-activity-template-contract.test.mjs \
  tests/static/academic-course-planning-contract.test.mjs \
  tests/static/academic-curriculum-core-contract.test.mjs \
  tests/static/activity-generate-state.test.mjs \
  tests/static/auto-scheduler-removal.test.mjs \
  tests/static/exam-schedule-export.test.mjs \
  tests/static/route-preview-meta.test.mjs \
  tests/static/timetable-teacher-load-export.test.mjs
```

- [ ] **Step 2: Verify repository formatting**

Run:

```bash
cd frontend-school && npx prettier --check .
```

Expected: exit 0 with `All matched files use Prettier code style!`.

- [ ] **Step 3: Commit formatting**

```bash
git add frontend-school
git commit -m "style(frontend): apply project formatting"
```

### Task 5: Refresh current project guidance

**Files:**
- Modify: `.rules`
- Modify: `IMPROVEMENT_PLAN.md`
- Modify: `docs/TESTING.md`
- Modify: `docs/backend-school/API_DEVELOPMENT.md`
- Modify: `docs/PROJECT_IMPROVEMENT_ANALYSIS_2026-07-21.md`
- Test: contract inventory and focused documentation search

**Interfaces:**
- Consumes: `contracts/openapi/school-api.json` operation IDs and completed 2026-07-23 plans.
- Produces: current guidance reporting 178 operations and resolved refactor/performance items.

- [ ] **Step 1: Update the operation checkpoint**

In `.rules`, `IMPROVEMENT_PLAN.md`, `docs/TESTING.md`, and
`docs/backend-school/API_DEVELOPMENT.md`:

- replace `177` with `178` only in the current checkpoint;
- replace activity-workspace `seven dependent reads` / `7 dependent reads`
  with `eight dependent reads` / `8 dependent reads`;
- add `getActivitySlotTimetableContext` immediately after `listActivitySlots`
  in the activity-workspace operation inventory.

- [ ] **Step 2: Add a dated status note to the improvement analysis**

Add this section near the top of
`docs/PROJECT_IMPROVEMENT_ANALYSIS_2026-07-21.md`:

```markdown
## Status update — 2026-07-23

The findings below preserve the 2026-07-21 analysis baseline. The following
items are now resolved:

- backend-school exposes a dependency-aware `/ready` endpoint;
- exam schedule, supervision, timetable, and calendar services have cohesive
  private modules behind stable service facades with focused tests;
- the timetable activity-slot fan-out is replaced by
  `GET /api/academic/activity-slots/timetable-context`;
- the frontend API client accepts `AbortSignal`, and timetable loading rejects
  stale request results through a request coordinator;
- the generated school API contract contains 178 unique operations.

Remaining recommendations should be evaluated against current code before
implementation. Historical measurements and findings stay unchanged below.
```

Update the recommended-roadmap checkboxes for `/ready`, timetable batch
endpoints, service modularization, OpenAPI-generated contracts, frontend
request cancellation, and documentation alignment to `[x]`.

- [ ] **Step 3: Verify the contract inventory and documentation**

Run:

```bash
node -e "const api=require('./contracts/openapi/school-api.json'); const operations=Object.values(api.paths).flatMap(path=>Object.values(path)).filter(operation=>operation&&typeof operation==='object'&&operation.operationId); if(operations.length!==178) process.exit(1); if(!operations.some(operation=>operation.operationId==='getActivitySlotTimetableContext')) process.exit(1);"
rg -n "177 unique operations|177-operation checkpoint|seven dependent reads|7 dependent reads" \
  .rules IMPROVEMENT_PLAN.md docs/TESTING.md docs/backend-school/API_DEVELOPMENT.md
```

Expected: Node exits 0; `rg` exits 1 with no stale current-checkpoint matches.

- [ ] **Step 4: Commit documentation**

```bash
git add .rules IMPROVEMENT_PLAN.md docs/TESTING.md \
  docs/backend-school/API_DEVELOPMENT.md \
  docs/PROJECT_IMPROVEMENT_ANALYSIS_2026-07-21.md
git commit -m "docs: refresh completed improvement status"
```

### Task 6: Run the complete verification gate

**Files:**
- Test: all modified frontend and documentation files

**Interfaces:**
- Consumes: completed Tasks 1–5.
- Produces: fresh evidence that lint, Svelte analysis, TypeScript, static contracts, API contracts, and diff hygiene pass.

- [ ] **Step 1: Re-run Svelte autofixer**

Run the official Svelte autofixer on every behavior-edited `.svelte` file until
each result contains no issue or suggestion.

- [ ] **Step 2: Run frontend quality and contract checks**

Run:

```bash
cd frontend-school
npm run lint
npm run check
npm run test:static
npm run check:api-contracts
npm run test:api-contracts
```

Expected: every command exits 0.

- [ ] **Step 3: Run repository diff checks**

Run:

```bash
git diff --check main...HEAD
git status --short
```

Expected: no whitespace errors; status contains no uncommitted implementation
files.

- [ ] **Step 4: Review the branch diff**

Run:

```bash
git diff --stat main...HEAD
git diff --name-status main...HEAD
```

Expected: changes are limited to the design/plan, frontend lint targets, and
the five current documentation files.
