# Daily Teaching Activity Cell Details Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align all daily timetable cards to 88px, show classroom and physical room for independent activities, preserve manually entered title line breaks, and naturally order synchronized activity locations in the detail dialog.

**Architecture:** Keep the existing typed daily-teaching API unchanged. Extend the pure display helper with structured, naturally sorted locations and make card presentation depend on both entry type and activity scheduling mode; then consume that view model in the existing Svelte page.

**Tech Stack:** SvelteKit 5, TypeScript, Tailwind CSS, shadcn-svelte, Node test runner, generated SchoolOrbit API types.

## Global Constraints

- All occupied cards use a fixed 88px height and hide overflow.
- Independent activities show both classroom and physical room when available.
- Synchronized detail rows sort naturally by classroom and then physical room.
- Non-course titles preserve embedded newlines and remain line-clamped.
- Keep the 128px teacher column and 132px minimum period columns unchanged.
- Keep wide-screen expansion, sticky headers, and narrow-screen horizontal scrolling unchanged.
- Do not change backend queries, OpenAPI/generated contracts, permissions, migrations, or realtime behavior.
- Run focused tests first, then the full frontend verification matrix from `.rules` and `docs/TESTING.md`.

---

### Task 1: Structured activity locations and scheduling-aware presentation

**Files:**
- Modify: `frontend-school/tests/static/daily-teaching-display.test.mjs`
- Modify: `frontend-school/src/lib/utils/daily-teaching-display.ts`

**Interfaces:**
- Consumes: generated `DailyTeachingEntry` fields `entryType`, `activitySchedulingMode`, `classroomName`, and `roomCode`.
- Produces: `DailyTeachingDisplayLocation`, `DailyTeachingDisplayGroup.locations`, and `dailyTeachingEntryCardPresentation(entry)`.
- Preserves: `DailyTeachingDisplayGroup.classroomLabels` and `displayGroupCountLabel(group)` for existing consumers.

- [ ] **Step 1: Write failing natural-order and deduplication assertions**

Change the synchronized merge test to provide unsorted numeric classrooms and rooms, then assert the structured result independently of implementation logic:

```js
test('sorts synchronized locations naturally by classroom then physical room', () => {
	const groups = groupDailyTeachingEntries([
		entry({ entryId: 'entry-10', classroomName: 'ม.1/10', roomCode: '120' }),
		entry({ entryId: 'entry-2b', classroomName: 'ม.1/2', roomCode: '115' }),
		entry({ entryId: 'entry-2a', classroomName: 'ม.1/2', roomCode: '101' }),
		entry({ entryId: 'entry-duplicate', classroomName: 'ม.1/2', roomCode: '101' })
	]);

	assert.deepEqual(groups[0].locations, [
		{ key: 'ม.1/2\u0000101', classroomName: 'ม.1/2', roomCode: '101', label: 'ม.1/2 / 101' },
		{ key: 'ม.1/2\u0000115', classroomName: 'ม.1/2', roomCode: '115', label: 'ม.1/2 / 115' },
		{ key: 'ม.1/10\u0000120', classroomName: 'ม.1/10', roomCode: '120', label: 'ม.1/10 / 120' }
	]);
	assert.deepEqual(groups[0].classroomLabels, [
		'ม.1/2 / 101',
		'ม.1/2 / 115',
		'ม.1/10 / 120'
	]);
	assert.equal(displayGroupCountLabel(groups[0]), '3 ห้อง');
});
```

Production mutation caught: removing numeric sorting, secondary room sorting, or pair deduplication makes the literal order/count fail.

- [ ] **Step 2: Write failing scheduling-aware presentation assertions**

Replace the string-only presentation calls with minimal real entry fixtures and add the independent activity case:

```js
test('uses detailed cards for courses and independent activities', () => {
	assert.deepEqual(
		dailyTeachingEntryCardPresentation(
			entry({ entryType: 'COURSE', activitySchedulingMode: null, activitySlotId: null })
		),
		{ tone: 'course', layout: 'details', titleLineLimit: 2 }
	);
	assert.deepEqual(
		dailyTeachingEntryCardPresentation(
			entry({ entryType: 'ACTIVITY', activitySchedulingMode: 'independent' })
		),
		{ tone: 'activity', layout: 'details', titleLineLimit: 2 }
	);
	assert.deepEqual(
		dailyTeachingEntryCardPresentation(entry({ entryType: 'ACTIVITY' })),
		{ tone: 'activity', layout: 'centered', titleLineLimit: 3 }
	);
});
```

Keep explicit assertions for the remaining centered types:

```js
assert.deepEqual(
	dailyTeachingEntryCardPresentation(entry({ entryType: 'ACADEMIC' })),
	{ tone: 'course', layout: 'centered', titleLineLimit: 3 }
);
assert.deepEqual(
	dailyTeachingEntryCardPresentation(entry({ entryType: 'HOMEROOM' })),
	{ tone: 'activity', layout: 'centered', titleLineLimit: 3 }
);
assert.deepEqual(
	dailyTeachingEntryCardPresentation(entry({ entryType: 'BREAK' })),
	{ tone: 'break', layout: 'centered', titleLineLimit: 3 }
);
```

Production mutation caught: treating independent activities as centered title-only cards makes this test fail.

- [ ] **Step 3: Run the focused test and verify RED**

Run:

```bash
cd frontend-school
node --test tests/static/daily-teaching-display.test.mjs
```

Expected: FAIL because `locations` is absent, numeric classroom order is insertion order, and `dailyTeachingEntryCardPresentation` does not accept scheduling mode or return `titleLineLimit`.

- [ ] **Step 4: Add the structured location view model**

In `daily-teaching-display.ts`, add:

```ts
export interface DailyTeachingDisplayLocation {
	key: string;
	classroomName: string | null;
	roomCode: string | null;
	label: string;
}

const thaiNaturalCollator = new Intl.Collator('th', {
	numeric: true,
	sensitivity: 'base'
});
```

Add `locations: DailyTeachingDisplayLocation[]` to `DailyTeachingDisplayGroup`. Replace the label-only append path with normalized location creation:

```ts
function textOrNull(value: string | null | undefined): string | null {
	const normalized = value?.trim();
	return normalized ? normalized : null;
}

function locationFromEntry(entry: DailyTeachingEntry): DailyTeachingDisplayLocation | null {
	const classroomName = textOrNull(entry.classroomName);
	const roomCode = textOrNull(entry.roomCode);
	if (!classroomName && !roomCode) return null;

	return {
		key: `${classroomName ?? ''}\u0000${roomCode ?? ''}`,
		classroomName,
		roomCode,
		label: [classroomName, roomCode].filter(Boolean).join(' / ')
	};
}

function compareLocations(
	left: DailyTeachingDisplayLocation,
	right: DailyTeachingDisplayLocation
): number {
	return (
		thaiNaturalCollator.compare(left.classroomName ?? '', right.classroomName ?? '') ||
		thaiNaturalCollator.compare(left.roomCode ?? '', right.roomCode ?? '') ||
		left.key.localeCompare(right.key)
	);
}
```

When adding an entry to a group, deduplicate by `location.key`, sort `locations` with `compareLocations`, and replace `classroomLabels` with `locations.map((location) => location.label)`. Initialize both arrays in `displayGroup`.

- [ ] **Step 5: Make card presentation scheduling-aware**

Extend the presentation type:

```ts
export type DailyTeachingTitleLineLimit = 2 | 3;

export type DailyTeachingEntryCardPresentation = {
	tone: DailyTeachingEntryTone;
	layout: DailyTeachingEntryLayout;
	titleLineLimit: DailyTeachingTitleLineLimit;
};
```

Change the function to accept the relevant real entry fields:

```ts
export function dailyTeachingEntryCardPresentation(
	entry: Pick<DailyTeachingEntry, 'entryType' | 'activitySchedulingMode'>
): DailyTeachingEntryCardPresentation {
	if (entry.entryType === 'COURSE') {
		return { tone: 'course', layout: 'details', titleLineLimit: 2 };
	}
	if (entry.entryType === 'ACTIVITY' && entry.activitySchedulingMode === 'independent') {
		return { tone: 'activity', layout: 'details', titleLineLimit: 2 };
	}

	switch (entry.entryType) {
		case 'BREAK':
			return { tone: 'break', layout: 'centered', titleLineLimit: 3 };
		case 'ACTIVITY':
		case 'HOMEROOM':
			return { tone: 'activity', layout: 'centered', titleLineLimit: 3 };
		default:
			return { tone: 'course', layout: 'centered', titleLineLimit: 3 };
	}
}
```

- [ ] **Step 6: Run focused tests and verify GREEN**

Run:

```bash
cd frontend-school
node --test tests/static/daily-teaching-display.test.mjs
```

Expected: all daily-teaching display tests PASS with zero failures.

- [ ] **Step 7: Commit the tested helper change**

```bash
git add frontend-school/src/lib/utils/daily-teaching-display.ts \
  frontend-school/tests/static/daily-teaching-display.test.mjs
git commit -m "feat: order daily teaching activity locations"
```

---

### Task 2: Equal-height dense cards and complete activity metadata

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte`

**Interfaces:**
- Consumes: `dailyTeachingEntryCardPresentation(entry)` and `DailyTeachingDisplayGroup.locations` from Task 1.
- Produces: fixed-height card rendering, independent activity metadata, multiline manual titles, and sorted synchronized detail rows.
- Preserves: existing `openCell`, dialog badges, semantic colors, filters, responsive table constants, and accessible empty-cell label.

- [ ] **Step 1: Update the presentation call and dense outer rhythm**

Pass the full entry:

```svelte
{@const presentation = dailyTeachingEntryCardPresentation(entry)}
```

Reduce the occupied-cell rhythm without changing the column variables:

```svelte
<Table.Cell class="daily-teaching-period-column px-1 py-1 align-top">
  <button class="... min-h-20 w-full rounded-md p-0.5 ...">
    <div class="space-y-0.5">
```

- [ ] **Step 2: Fix every entry block at 88px**

Replace the minimum-height card base with:

```svelte
'h-[5.5rem] overflow-hidden rounded-md border p-2'
```

Keep the current semantic blue, emerald, and amber classes. Keep centered flex classes only when `presentation.layout === 'centered'`.

- [ ] **Step 3: Render course and independent activity details separately**

Inside the detailed layout branch, keep the current course code/name block only for `COURSE`. For an independent activity, render the manually entered title with preserved newlines:

```svelte
{#if entry.entryType === 'COURSE'}
	<!-- existing subject code and subject name rows -->
{:else}
	<p class="line-clamp-2 whitespace-pre-line text-xs font-semibold leading-snug">
		{entryTitle(entry)}
	</p>
{/if}
```

Reuse the existing School and MapPin metadata rows for both course and independent activity entries. Omit each row when its value is absent.

- [ ] **Step 4: Preserve multiline titles in centered cards**

Use the presentation line limit while always preserving explicit newlines:

```svelte
<p
	class={[
		'whitespace-pre-line text-sm font-semibold leading-snug',
		presentation.titleLineLimit === 2 ? 'line-clamp-2' : 'line-clamp-3'
	]}
>
	{entryTitle(entry)}
</p>
```

Keep the synchronized classroom-count line under the title.

- [ ] **Step 5: Replace synchronized badges with an ordered two-column list**

In the synchronized dialog branch, render `group.locations`:

```svelte
{#if group.locations.length > 0}
	<div class="mt-2 overflow-hidden rounded-md border">
		<div
			class="bg-muted/50 text-muted-foreground grid grid-cols-[minmax(0,1fr)_5rem] gap-2 px-3 py-1.5 text-xs"
		>
			<span>ชั้น/ห้อง</span>
			<span>ห้องเรียน</span>
		</div>
		{#each group.locations as location (location.key)}
			<div
				class="grid grid-cols-[minmax(0,1fr)_5rem] gap-2 border-t px-3 py-2 text-sm"
			>
				<span class="truncate">{location.classroomName ?? '-'}</span>
				<span class="truncate">{location.roomCode ?? '-'}</span>
			</div>
		{/each}
	</div>
{:else}
	<p class="mt-1 text-sm">{displayGroupCountLabel(group)}</p>
{/if}
```

Remove only the old synchronized `group.classroomLabels` badge loop. Keep the type, team-teaching, and synchronized status badges above it.

- [ ] **Step 6: Format and analyze the Svelte file**

Run:

```bash
cd frontend-school
npx prettier --write \
  'src/routes/(app)/staff/academic/timetable/today/+page.svelte' \
  src/lib/utils/daily-teaching-display.ts \
  tests/static/daily-teaching-display.test.mjs
npx @sveltejs/mcp svelte-autofixer \
  'src/routes/(app)/staff/academic/timetable/today/+page.svelte' \
  --svelte-version 5
```

Expected: the autofixer reports no Svelte issues. Resolve every reported issue before continuing.

- [ ] **Step 7: Run the focused regression suite**

Run:

```bash
cd frontend-school
node --test tests/static/daily-teaching-display.test.mjs
```

Expected: all daily-teaching display tests PASS.

- [ ] **Step 8: Commit the page refinement**

```bash
git add 'frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte'
git commit -m "style: align daily teaching activity cards"
```

---

### Task 3: Full frontend verification and final review

**Files:**
- Review: `frontend-school/src/lib/utils/daily-teaching-display.ts`
- Review: `frontend-school/tests/static/daily-teaching-display.test.mjs`
- Review: `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte`

**Interfaces:**
- Consumes: the complete Task 1 and Task 2 implementation.
- Produces: verification evidence and a clean committed working tree on `main`.

- [ ] **Step 1: Run the frontend verification matrix**

From `frontend-school` run:

```bash
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:static
```

Expected: every command exits 0; Svelte reports 0 errors and 0 warnings; the test summaries report 0 failures.

- [ ] **Step 2: Review requirements against the final code**

Confirm all of the following directly in the final diff:

```text
All occupied cards are h-[5.5rem] and overflow-hidden.
Independent activity cards show classroomName and roomCode.
Non-course titles use whitespace-pre-line and a line clamp.
Synchronized dialog rows use group.locations and natural helper order.
Type/team/synchronized dialog badges remain present.
Teacher and period width constants remain 128 and 132.
The table still uses overflow-x-auto.
No backend, generated contract, permission, or migration file changed.
```

- [ ] **Step 3: Check diff hygiene and repository state**

From the repository root run:

```bash
git diff --check
git status --short --branch
git log -5 --oneline --decorate
```

Expected: no whitespace errors, no uncommitted implementation files, and the new commits are visible on `main`. Do not push without explicit user authorization.
