# Daily Teaching Simple Cells Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace nested neutral cards and grid badges with simple semantic pastel blocks matching the main timetable's scan-friendly visual language.

**Architecture:** Extend the existing pure daily-teaching display helper with an entry-type presentation model and accessible empty-cell label, both covered by behavior tests. The Svelte page consumes those stable values to render three explicit Tailwind tone families while preserving grouping, dialogs, filters, responsive widths, and keyboard interaction.

**Tech Stack:** Svelte 5, TypeScript, Tailwind CSS 4, lucide-svelte, Node.js built-in test runner, local shadcn-svelte table and dialog components

## Global Constraints

- Work directly on the existing `main` branch; do not create a worktree.
- Keep the 128px teacher column, 132px minimum period columns, full-width expansion, sticky headers, and narrow-device horizontal scrolling unchanged.
- Remove type, team-teaching, and synchronized badges from the grid only; keep them in the period detail dialog.
- Map `COURSE` to blue/details, `ACADEMIC` to blue/centered, `ACTIVITY` and `HOMEROOM` to green/centered, and `BREAK` to amber/centered.
- Keep synchronized activity grouping and show its classroom count beneath the centered title.
- Render empty cells without visible text but preserve a descriptive accessible name and the existing click/keyboard dialog behavior.
- Use explicit light and dark Tailwind classes; do not construct class names dynamically.
- Preserve filters, summaries, raw dialog data, API contracts, backend behavior, permissions, database schema, realtime behavior, and the main timetable page.
- Use TDD: observe focused behavior tests failing before changing the display helper or Svelte markup.

---

### Task 1: Semantic daily-teaching entry blocks

**Files:**
- Modify: `frontend-school/tests/static/daily-teaching-display.test.mjs`
- Modify: `frontend-school/src/lib/utils/daily-teaching-display.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte:23-38, 674-736`

**Interfaces:**
- Consumes: `DailyTeachingEntry['entryType']`, teacher display name, `periodLabel(period)`, `periodTime(period)`, existing `DailyTeachingDisplayGroup`, and raw cell entries.
- Produces: `DailyTeachingEntryTone = 'course' | 'activity' | 'break'`, `DailyTeachingEntryLayout = 'details' | 'centered'`, `dailyTeachingEntryCardPresentation(entryType): { tone; layout }`, and `dailyTeachingEmptyCellLabel(teacherName, periodName, periodTime): string`.

- [ ] **Step 1: Add failing behavior tests for semantic presentation**

In `frontend-school/tests/static/daily-teaching-display.test.mjs`, add the wished-for exports to the existing namespace destructure:

```js
const {
	DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH,
	DAILY_TEACHING_TEACHER_COLUMN_WIDTH,
	dailyTeachingEmptyCellLabel,
	dailyTeachingEntryCardPresentation,
	dailyTeachingTableMinWidth,
	dailyTeachingTeacherCell,
	displayGroupCountLabel,
	groupDailyTeachingEntries
} = dailyTeachingDisplay;
```

Append these behavior tests:

```js
test('maps every daily entry type to a semantic tone and content layout', () => {
	assert.equal(typeof dailyTeachingEntryCardPresentation, 'function');
	assert.deepEqual(dailyTeachingEntryCardPresentation('COURSE'), {
		tone: 'course',
		layout: 'details'
	});
	assert.deepEqual(dailyTeachingEntryCardPresentation('ACADEMIC'), {
		tone: 'course',
		layout: 'centered'
	});
	assert.deepEqual(dailyTeachingEntryCardPresentation('ACTIVITY'), {
		tone: 'activity',
		layout: 'centered'
	});
	assert.deepEqual(dailyTeachingEntryCardPresentation('HOMEROOM'), {
		tone: 'activity',
		layout: 'centered'
	});
	assert.deepEqual(dailyTeachingEntryCardPresentation('BREAK'), {
		tone: 'break',
		layout: 'centered'
	});
});

test('builds an accessible label for a visually empty period cell', () => {
	assert.equal(typeof dailyTeachingEmptyCellLabel, 'function');
	assert.equal(
		dailyTeachingEmptyCellLabel('วิภาวดี วงศ์ศรี', 'คาบที่ 1', '08:40-09:30'),
		'วิภาวดี วงศ์ศรี คาบที่ 1 08:40-09:30: ว่าง'
	);
});
```

- [ ] **Step 2: Run the focused tests and verify RED**

Run from `frontend-school`:

```bash
node --test tests/static/daily-teaching-display.test.mjs
```

Expected: the existing 8 tests pass and the 2 new tests fail because both wished-for functions are `undefined`.

- [ ] **Step 3: Implement the pure presentation model**

Add these exported types after the existing teacher identity type in `frontend-school/src/lib/utils/daily-teaching-display.ts`:

```ts
export type DailyTeachingEntryTone = 'course' | 'activity' | 'break';
export type DailyTeachingEntryLayout = 'details' | 'centered';

export type DailyTeachingEntryCardPresentation = {
	tone: DailyTeachingEntryTone;
	layout: DailyTeachingEntryLayout;
};
```

Append the functions after `dailyTeachingTeacherCell`:

```ts
export function dailyTeachingEntryCardPresentation(
	entryType: DailyTeachingEntry['entryType']
): DailyTeachingEntryCardPresentation {
	switch (entryType) {
		case 'COURSE':
			return { tone: 'course', layout: 'details' };
		case 'BREAK':
			return { tone: 'break', layout: 'centered' };
		case 'ACTIVITY':
		case 'HOMEROOM':
			return { tone: 'activity', layout: 'centered' };
		default:
			return { tone: 'course', layout: 'centered' };
	}
}

export function dailyTeachingEmptyCellLabel(
	teacherName: string,
	periodName: string,
	periodTime: string
): string {
	return `${teacherName} ${periodName} ${periodTime}: ว่าง`;
}
```

The `default` branch intentionally owns `ACADEMIC`, the only remaining generated union member, and provides a safe blue/centered fallback if the contract later adds another non-course type.

- [ ] **Step 4: Run the focused tests and verify GREEN**

Run from `frontend-school`:

```bash
node --test tests/static/daily-teaching-display.test.mjs
```

Expected: PASS, 10 tests.

- [ ] **Step 5: Import the presentation helpers and metadata icons**

In `+page.svelte`, add `dailyTeachingEmptyCellLabel` and `dailyTeachingEntryCardPresentation` to the existing daily-teaching display import.

Add `MapPin` and `School` to the existing `lucide-svelte` import:

```ts
	import {
		CalendarClock,
		ChevronLeft,
		ChevronRight,
		ExternalLink,
		MapPin,
		Printer,
		RefreshCw,
		School,
		Search
	} from 'lucide-svelte';
```

- [ ] **Step 6: Replace the nested badge card with semantic flat blocks**

Keep the period cell as one button. Replace its class and body with this structure:

```svelte
<button
	type="button"
	class="hover:bg-muted/30 focus-visible:border-ring focus-visible:ring-ring/50 min-h-20 w-full rounded-md p-1 text-left transition-colors focus-visible:ring-[3px] focus-visible:outline-none"
	aria-label={cell.entries.length === 0
		? dailyTeachingEmptyCellLabel(
				teacherCell.label,
				periodLabel(period),
				periodTime(period)
			)
		: undefined}
	onclick={() => openCell(teacher, period, cell.entries)}
>
	{#if cell.entries.length > 0}
		<div class="space-y-1">
			{#each displayGroups as group (group.key)}
				{@const entry = group.entries[0]}
				{@const presentation = dailyTeachingEntryCardPresentation(entry.entryType)}
				{@const subjectCodeLine = entrySubjectCodeLine(entry)}
				{@const subjectNameLine = entrySubjectNameLine(entry)}
				<div
					class={[
						'min-h-16 rounded-md border p-2',
						presentation.layout === 'centered' &&
							'flex flex-col items-center justify-center text-center',
						presentation.tone === 'course' &&
							'border-blue-200 bg-blue-50/80 text-blue-950 dark:border-blue-800 dark:bg-blue-950/40 dark:text-blue-100',
						presentation.tone === 'activity' &&
							'border-emerald-200 bg-emerald-50/80 text-emerald-950 dark:border-emerald-800 dark:bg-emerald-950/40 dark:text-emerald-100',
						presentation.tone === 'break' &&
							'border-amber-300 bg-amber-50/90 text-amber-900 dark:border-amber-800 dark:bg-amber-950/40 dark:text-amber-100'
					]}
				>
					{#if presentation.layout === 'details'}
						{#if subjectCodeLine}
							<p class="truncate text-xs font-bold">{subjectCodeLine}</p>
						{/if}
						{#if subjectNameLine}
							<p class="line-clamp-2 text-xs opacity-75">{subjectNameLine}</p>
						{/if}
						{#if entry.classroomName || entry.roomCode}
							<div class="mt-1.5 space-y-0.5 border-t border-current/10 pt-1 text-[10px] opacity-70">
								{#if entry.classroomName}
									<p class="flex items-center gap-1 truncate">
										<School class="h-3 w-3 shrink-0" />
										<span class="truncate">{entry.classroomName}</span>
									</p>
								{/if}
								{#if entry.roomCode}
									<p class="flex items-center gap-1 truncate">
										<MapPin class="h-3 w-3 shrink-0" />
										<span class="truncate">{entry.roomCode}</span>
									</p>
								{/if}
							</div>
						{/if}
					{:else}
						<p class="line-clamp-3 text-sm font-semibold leading-snug">
							{entryTitle(entry)}
						</p>
						{#if group.isSynchronizedActivity}
							<p class="mt-1 truncate text-[10px] opacity-70">
								{displayGroupCountLabel(group)}
							</p>
						{/if}
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</button>
```

Delete the grid-only calls to `entryBadgeVariant`, `entryTypeLabel`, and their three `<Badge>` elements from this block. Keep both functions because the detail dialog still uses them. Keep raw entries, grouping, click behavior, and dialog markup unchanged.

Delete the now-unused `entryMeta(entry)` helper from the page; classroom and room metadata are rendered separately in the new course block.

- [ ] **Step 7: Re-run focused behavior and architecture tests**

Run from `frontend-school`:

```bash
node --test tests/static/daily-teaching-display.test.mjs tests/static/api-global-contract.test.mjs
```

Expected: PASS for all tests in both files. The architecture test continues to see subject code/name helpers, responsive width helpers, sticky table structure, and the read-only dialog path.

- [ ] **Step 8: Run Svelte analysis and format the touched files**

Run from `frontend-school`:

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/timetable/today/+page.svelte' --svelte-version 5
npx prettier --write src/lib/utils/daily-teaching-display.ts tests/static/daily-teaching-display.test.mjs 'src/routes/(app)/staff/academic/timetable/today/+page.svelte'
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/timetable/today/+page.svelte' --svelte-version 5
```

Expected: no Svelte issues. Existing advisory suggestions about the page's pre-existing effects may remain; do not refactor unrelated loading behavior.

- [ ] **Step 9: Run the frontend verification matrix**

Run from `frontend-school`:

```bash
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

Expected: formatting/ESLint pass, `svelte-check` reports 0 errors and 0 warnings, and all static tests pass.

- [ ] **Step 10: Review and commit the implementation**

Run from the repository root:

```bash
git diff --check
git diff -- frontend-school/src/lib/utils/daily-teaching-display.ts frontend-school/tests/static/daily-teaching-display.test.mjs 'frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte'
git status --short
```

Confirm that the diff changes only grid presentation and its pure behavior tests, then commit:

```bash
git add frontend-school/src/lib/utils/daily-teaching-display.ts frontend-school/tests/static/daily-teaching-display.test.mjs 'frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte'
git commit -m "style: simplify daily teaching cells"
```
