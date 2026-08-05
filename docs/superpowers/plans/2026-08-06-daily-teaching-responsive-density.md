# Daily Teaching Responsive Density Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the daily teaching table fit all periods on wide screens, retain horizontal scrolling on smaller screens, and reduce row noise by removing the subject-group subtitle beneath teacher names.

**Architecture:** Extend the existing daily-teaching display helper with the pure width calculation and teacher-cell presentation contract, cover both through behavior tests, then consume them from the existing Svelte page. The table uses a 128px sticky teacher column, a 132px readable minimum for each period, `width: 100%`, and fixed table layout so spare desktop width is shared while narrow viewports overflow the existing scroll container.

**Tech Stack:** Svelte 5, TypeScript, Tailwind CSS 4, Node.js built-in test runner, local shadcn-svelte table components

## Global Constraints

- Work directly on the existing `main` branch; do not create a worktree.
- Keep the sticky teacher column at exactly 128px.
- Keep every period at a readable minimum of exactly 132px.
- Wide screens fill the available schedule width; smaller screens retain horizontal scrolling.
- Remove subject-group names only from teacher rows. Preserve subject-group search, filtering, API data, and period details.
- Preserve current colors, typography, badges, synchronized-activity grouping, focus behavior, dialogs, summary calculations, and permissions.
- Do not change backend code, API contracts, generated files, database migrations, realtime behavior, or canonical documentation.
- Use TDD: add and observe the focused regression test failing before editing the Svelte page.

---

### Task 1: Responsive compact daily teaching table

**Files:**
- Modify: `frontend-school/src/lib/utils/daily-teaching-display.ts`
- Modify: `frontend-school/tests/static/daily-teaching-display.test.mjs`
- Modify: `frontend-school/tests/static/api-global-contract.test.mjs:1184-1187`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte:133-144, 623-753, 839-859`

**Interfaces:**
- Consumes: `DailyTeachingOverview.periods`, `DailyTeachingTeacher.displayName`, existing `.daily-teaching-scroll`, `.daily-teaching-table`, `.daily-teaching-teacher-column`, and `.daily-teaching-period-column` hooks.
- Produces: `DAILY_TEACHING_TEACHER_COLUMN_WIDTH: 128`, `DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH: 132`, `dailyTeachingTableMinWidth(periodCount: number): number`, and `dailyTeachingTeacherCell(teacher: { displayName: string }): { label: string; title: string }`.

- [ ] **Step 1: Add failing behavior tests for the layout model**

In `frontend-school/tests/static/daily-teaching-display.test.mjs`, replace the named import with a namespace import so wished-for exports fail as assertions instead of a module-loading error:

```js
import assert from 'node:assert/strict';
import test from 'node:test';

import * as dailyTeachingDisplay from '../../src/lib/utils/daily-teaching-display.ts';

const {
	DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH,
	DAILY_TEACHING_TEACHER_COLUMN_WIDTH,
	dailyTeachingTableMinWidth,
	dailyTeachingTeacherCell,
	displayGroupCountLabel,
	groupDailyTeachingEntries
} = dailyTeachingDisplay;
```

Append these tests after the existing synchronized-activity display tests:

```js

test('calculates the readable table minimum from the teacher and period columns', () => {
	assert.equal(DAILY_TEACHING_TEACHER_COLUMN_WIDTH, 128);
	assert.equal(DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH, 132);
	assert.equal(typeof dailyTeachingTableMinWidth, 'function');
	assert.equal(dailyTeachingTableMinWidth(0), 128);
	assert.equal(dailyTeachingTableMinWidth(4), 656);
	assert.equal(dailyTeachingTableMinWidth(10), 1448);
});

test('teacher cell presentation excludes subject-group subtitles', () => {
	assert.equal(typeof dailyTeachingTeacherCell, 'function');
	assert.deepEqual(
		dailyTeachingTeacherCell({
			displayName: 'วิภาวดี วงศ์ศรี',
			subjectGroupNames: ['ภาษาไทย']
		}),
		{
			label: 'วิภาวดี วงศ์ศรี',
			title: 'วิภาวดี วงศ์ศรี'
		}
	);
});
```

- [ ] **Step 2: Run the focused test and verify RED**

Run from `frontend-school`:

```bash
node --test tests/static/daily-teaching-display.test.mjs
```

Expected: FAIL with `undefined !== 128` and `undefined !== 'function'` because the existing display helper does not export the responsive layout model yet. The six pre-existing grouping tests continue to pass.

- [ ] **Step 3: Implement the pure layout model**

Append the pure layout model to `frontend-school/src/lib/utils/daily-teaching-display.ts`:

```ts
export const DAILY_TEACHING_TEACHER_COLUMN_WIDTH = 128;
export const DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH = 132;

type DailyTeachingTeacherIdentity = {
	displayName: string;
};

export function dailyTeachingTableMinWidth(periodCount: number): number {
	return (
		DAILY_TEACHING_TEACHER_COLUMN_WIDTH +
		periodCount * DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH
	);
}

export function dailyTeachingTeacherCell(teacher: DailyTeachingTeacherIdentity): {
	label: string;
	title: string;
} {
	return {
		label: teacher.displayName,
		title: teacher.displayName
	};
}
```

- [ ] **Step 4: Run the layout-model tests and verify GREEN**

Run from `frontend-school`:

```bash
node --test tests/static/daily-teaching-display.test.mjs
```

Expected: PASS, 8 tests.

- [ ] **Step 5: Consume the layout model in the Svelte table**

Import the layout model next to the existing daily-teaching display helper:

```ts
	import {
		DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH,
		DAILY_TEACHING_TEACHER_COLUMN_WIDTH,
		dailyTeachingTableMinWidth,
		dailyTeachingTeacherCell
	} from '$lib/utils/daily-teaching-display';
```

Replace the dynamic teacher-width block, fixed period width, and unused `clamp` helper with:

```ts
	const tableMinWidth = $derived(dailyTeachingTableMinWidth(overview?.periods.length ?? 4));
```

Bind the pure layout constants to CSS custom properties and keep the calculated table minimum:

```svelte
style={`--teacher-column-width: ${DAILY_TEACHING_TEACHER_COLUMN_WIDTH}px; --minimum-period-column-width: ${DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH}px; min-width: ${tableMinWidth}px;`}
```

Add a full-width fixed-layout rule before the existing table cell rules:

```css
	:global(.daily-teaching-table) {
		table-layout: fixed;
		width: 100%;
	}
```

Replace the period-column rule with the exact readable minimum:

```css
	:global(.daily-teaching-period-column) {
		width: var(--minimum-period-column-width);
		min-width: var(--minimum-period-column-width);
	}
```

Do not set `max-width` on period columns: when the viewport is wider than the calculated table minimum, fixed table layout distributes remaining width across the periods.

- [ ] **Step 6: Remove the teacher subtitle and compact the table presentation**

Inside the keyed teacher row, derive the presentation once:

```svelte
{@const teacherCell = dailyTeachingTeacherCell(teacher)}
```

Replace the teacher-cell body with a single truncated name that preserves the full value through `title`:

```svelte
<div class="min-w-0" title={teacherCell.title}>
	<p class="truncate font-medium">{teacherCell.label}</p>
</div>
```

Keep all `subjectGroupNames` usage in search and filter derivations unchanged. Keep the subject-group filter with `id="subject-group-filter"` unchanged.

Apply these compact class changes:

- schedule heading container: `p-4` → `p-3 sm:p-4`;
- teacher header/cell and period header/cell: add `px-1.5 py-2`;
- period button: `min-h-24 ... p-2` → `min-h-20 ... p-1.5`;
- display-group stack: `space-y-1.5` → `space-y-1`;
- inner entry card: `rounded-md border bg-muted/30 p-2` → `rounded-md border bg-muted/30 p-1.5`.

Keep the existing badges, font sizes, line clamps, focus classes, click handler, sticky positions, and dialog markup unchanged.

- [ ] **Step 7: Update the existing daily-page architecture guard**

In `frontend-school/tests/static/api-global-contract.test.mjs`, replace the old dynamic-name-width expectations:

```js
	assert.match(page, /teacherColumnWidth/);
	assert.match(page, /teacher\.displayName\.length/);
```

with the responsive layout contract:

```js
	assert.match(page, /DAILY_TEACHING_TEACHER_COLUMN_WIDTH/);
	assert.match(page, /DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH/);
	assert.match(page, /dailyTeachingTableMinWidth/);
	assert.doesNotMatch(page, /teacher\.displayName\.length/);
```

Keep the existing checks for table structure, sticky headers, subject-group filtering data, dialogs, and read-only behavior unchanged.

- [ ] **Step 8: Re-run the focused behavior and architecture tests**

Run from `frontend-school`:

```bash
node --test tests/static/daily-teaching-display.test.mjs tests/static/api-global-contract.test.mjs
```

Expected: PASS for all tests in both files.

- [ ] **Step 9: Run Svelte analysis and resolve issues introduced by this change**

Run from `frontend-school`:

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/timetable/today/+page.svelte' --svelte-version 5
```

Expected: no issues. Existing advisory suggestions about the page's pre-existing effects may remain; do not refactor unrelated data-loading behavior.

- [ ] **Step 10: Run the frontend verification matrix**

Run from `frontend-school`:

```bash
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

Expected: formatting/ESLint pass, `svelte-check` reports 0 errors and 0 warnings, and all static tests pass.

- [ ] **Step 11: Review the final change and commit**

Run from the repository root:

```bash
git diff --check
git diff -- frontend-school/src/lib/utils/daily-teaching-display.ts frontend-school/tests/static/daily-teaching-display.test.mjs frontend-school/tests/static/api-global-contract.test.mjs 'frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte'
git status --short
```

Confirm the diff changes only the responsive-density behavior described above, then commit:

```bash
git add frontend-school/src/lib/utils/daily-teaching-display.ts frontend-school/tests/static/daily-teaching-display.test.mjs frontend-school/tests/static/api-global-contract.test.mjs 'frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte'
git commit -m "fix: fit daily teaching periods on wide screens"
```
