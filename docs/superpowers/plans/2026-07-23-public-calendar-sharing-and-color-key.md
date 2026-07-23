# Public Calendar Sharing And Color Key Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a staff action that copies the current school's public calendar URL and show a reusable, month-scoped `คำอธิบายสี` row on the public calendar.

**Architecture:** Keep the change frontend-only. Derive public color-key items from the public events already loaded for the calendar grid, isolate the month-filter/deduplication rules in a pure utility, and render those items through one typed Svelte component shared by staff and public pages.

**Tech Stack:** SvelteKit 5, TypeScript, Svelte runes, Tailwind CSS, local shadcn-svelte `Button`, `svelte-sonner`, Node test runner, Playwright.

## Global Constraints

- The visible UI term is `คำอธิบายสี`; do not display the English term “legend”.
- Public color items include only categories used by public events overlapping the selected calendar month.
- Adjacent-month-only events loaded for the 42-day grid must not affect the selected month's color key.
- Uncategorized public events use `ไม่ระบุหมวดหมู่` with `#64748b`, placed after named categories.
- Do not add or change backend routes, OpenAPI contracts, database migrations, permissions, or dependencies.
- Preserve the public page's `h-dvh` and document-level no-scroll behavior.
- Preserve the mobile day timeline dialog and desktop selected-day panel.
- Write each behavior test first and observe the expected failure before production changes.

---

## File Structure

- Modify `frontend-school/src/lib/utils/calendar.ts`: own the pure month filtering, deduplication, sorting, and fallback rules.
- Modify `frontend-school/tests/static/calendar-utils.test.mjs`: exercise the utility with real event inputs.
- Create `frontend-school/src/lib/components/calendar/CalendarColorKey.svelte`: render the reusable Thai color key.
- Modify `frontend-school/tests/static/calendar.test.mjs`: guard shared-component usage, clipboard behavior, and public integration.
- Modify `frontend-school/src/routes/(app)/staff/calendar/+page.svelte`: add the copy action and replace its inline category dots.
- Modify `frontend-school/src/routes/(public)/calendar/+page.svelte`: derive and render month-scoped public color items.

### Task 1: Month-Scoped Color-Key Utility

**Files:**
- Modify: `frontend-school/tests/static/calendar-utils.test.mjs`
- Modify: `frontend-school/src/lib/utils/calendar.ts`

**Interfaces:**
- Produces:
  - `CalendarColorKeyEvent`
  - `CalendarColorKeyItem`
  - `CALENDAR_FALLBACK_COLOR`
  - `buildCalendarColorKey(monthDate: string, events: CalendarColorKeyEvent[]): CalendarColorKeyItem[]`

- [ ] **Step 1: Write failing utility tests**

Add `buildCalendarColorKey` to the existing import and append:

```js
it('builds a selected-month color key without adjacent-month-only events', () => {
	const items = buildCalendarColorKey('2026-07-15', [
		{
			id: 'june',
			startDate: '2026-06-30',
			endDate: '2026-06-30',
			categoryId: 'internal',
			categoryName: 'ภายใน',
			categoryColor: '#111827'
		},
		{
			id: 'spanning',
			startDate: '2026-06-29',
			endDate: '2026-07-02',
			categoryId: 'camp',
			categoryName: 'ค่าย',
			categoryColor: '#7c3aed'
		},
		{
			id: 'july',
			startDate: '2026-07-20',
			endDate: '2026-07-20',
			categoryId: 'academic',
			categoryName: 'วิชาการ',
			categoryColor: '#0284c7'
		}
	]);

	assert.deepEqual(items, [
		{ id: 'camp', name: 'ค่าย', color: '#7c3aed' },
		{ id: 'academic', name: 'วิชาการ', color: '#0284c7' }
	]);
});

it('deduplicates categories and places the uncategorized fallback last', () => {
	const items = buildCalendarColorKey('2026-07-01', [
		{
			id: 'academic-1',
			startDate: '2026-07-01',
			endDate: '2026-07-01',
			categoryId: 'academic',
			categoryName: 'วิชาการ',
			categoryColor: '#0284c7'
		},
		{
			id: 'academic-2',
			startDate: '2026-07-02',
			endDate: '2026-07-02',
			categoryId: 'academic',
			categoryName: 'วิชาการ',
			categoryColor: '#0284c7'
		},
		{
			id: 'meeting',
			startDate: '2026-07-03',
			endDate: '2026-07-03',
			categoryId: 'meeting',
			categoryName: 'ประชุม',
			categoryColor: '#16a34a'
		},
		{
			id: 'uncategorized',
			startDate: '2026-07-04',
			endDate: '2026-07-04',
			categoryId: null,
			categoryName: null,
			categoryColor: null
		}
	]);

	assert.deepEqual(items, [
		{ id: 'meeting', name: 'ประชุม', color: '#16a34a' },
		{ id: 'academic', name: 'วิชาการ', color: '#0284c7' },
		{ id: 'uncategorized', name: 'ไม่ระบุหมวดหมู่', color: '#64748b' }
	]);
});
```

- [ ] **Step 2: Run the utility test and verify RED**

Run:

```bash
cd frontend-school
node --test tests/static/calendar-utils.test.mjs
```

Expected: FAIL because `buildCalendarColorKey` is not exported.

- [ ] **Step 3: Implement the minimal pure utility**

Add to `calendar.ts`:

```ts
export const CALENDAR_FALLBACK_COLOR = '#64748b';

export interface CalendarColorKeyEvent {
	id: string;
	startDate: string;
	endDate: string;
	categoryId?: string | null;
	categoryName?: string | null;
	categoryColor?: string | null;
}

export interface CalendarColorKeyItem {
	id: string;
	name: string;
	color: string;
}

export function buildCalendarColorKey(
	monthDate: string,
	events: CalendarColorKeyEvent[]
): CalendarColorKeyItem[] {
	const range = monthRange(monthDate);
	const categories = new Map<string, CalendarColorKeyItem>();
	let hasUncategorizedEvent = false;

	for (const event of events) {
		if (event.startDate > range.to || event.endDate < range.from) continue;

		if (event.categoryId && event.categoryName && event.categoryColor) {
			if (!categories.has(event.categoryId)) {
				categories.set(event.categoryId, {
					id: event.categoryId,
					name: event.categoryName,
					color: event.categoryColor
				});
			}
		} else {
			hasUncategorizedEvent = true;
		}
	}

	const items = [...categories.values()].sort((left, right) =>
		left.name.localeCompare(right.name, 'th')
	);

	if (hasUncategorizedEvent) {
		items.push({
			id: 'uncategorized',
			name: 'ไม่ระบุหมวดหมู่',
			color: CALENDAR_FALLBACK_COLOR
		});
	}

	return items;
}
```

- [ ] **Step 4: Run the utility test and verify GREEN**

Run:

```bash
cd frontend-school
node --test tests/static/calendar-utils.test.mjs
```

Expected: all calendar helper tests pass.

- [ ] **Step 5: Commit the utility slice**

```bash
git add frontend-school/src/lib/utils/calendar.ts frontend-school/tests/static/calendar-utils.test.mjs
git commit -m "feat(calendar): derive public color key"
```

### Task 2: Shared Thai Color-Key Component

**Files:**
- Create: `frontend-school/src/lib/components/calendar/CalendarColorKey.svelte`
- Modify: `frontend-school/tests/static/calendar.test.mjs`
- Modify: `frontend-school/src/routes/(app)/staff/calendar/+page.svelte`

**Interfaces:**
- Consumes: `CalendarColorKeyItem[]`
- Produces: `<CalendarColorKey items={...} />`

- [ ] **Step 1: Write the failing shared-component integration test**

In the shared-components test, read `CalendarColorKey.svelte` and assert:

```js
const colorKey = await readProjectFile(
	'src/lib/components/calendar/CalendarColorKey.svelte'
);
assert.match(colorKey, /CalendarColorKeyItem/);
assert.match(colorKey, /คำอธิบายสี/);
assert.match(colorKey, /\{#each items as item \(item\.id\)\}/);
assert.match(colorKey, /overflow-x-auto/);
assert.match(colorKey, /sm:flex-wrap/);

const staffPage = await readProjectFile('src/routes/(app)/staff/calendar/+page.svelte');
assert.match(staffPage, /CalendarColorKey/);
assert.match(staffPage, /items=\{activeCategories\}/);
```

- [ ] **Step 2: Run the static test and verify RED**

Run:

```bash
cd frontend-school
node --test tests/static/calendar.test.mjs
```

Expected: FAIL because `CalendarColorKey.svelte` does not exist.

- [ ] **Step 3: Create the typed Svelte component**

Create:

```svelte
<script lang="ts">
	import type { CalendarColorKeyItem } from '$lib/utils/calendar';

	let { items = [] }: { items?: CalendarColorKeyItem[] } = $props();
</script>

<div
	class="flex shrink-0 items-center gap-3 overflow-x-auto px-1 pb-1 text-xs text-muted-foreground sm:flex-wrap sm:overflow-visible"
	aria-label="คำอธิบายสีปฏิทิน"
>
	<span class="shrink-0 font-medium text-foreground/80">คำอธิบายสี</span>
	<div class="flex min-w-max items-center gap-4 sm:min-w-0 sm:flex-wrap">
		{#each items as item (item.id)}
			<span class="flex shrink-0 items-center gap-1.5 whitespace-nowrap">
				<span
					class="size-2 rounded-full"
					style:background-color={item.color}
					aria-hidden="true"
				></span>
				{item.name}
			</span>
		{/each}
	</div>
</div>
```

- [ ] **Step 4: Replace the staff page's inline color dots**

Import `CalendarColorKey` and replace the existing inline `{#each activeCategories ...}` block with:

```svelte
{#if activeCategories.length > 0}
	<CalendarColorKey items={activeCategories} />
{/if}
```

- [ ] **Step 5: Format and verify GREEN**

Run:

```bash
cd frontend-school
npx prettier --write src/lib/components/calendar/CalendarColorKey.svelte 'src/routes/(app)/staff/calendar/+page.svelte' tests/static/calendar.test.mjs
node --test tests/static/calendar.test.mjs
```

Expected: all calendar static tests pass.

- [ ] **Step 6: Commit the shared component slice**

```bash
git add frontend-school/src/lib/components/calendar/CalendarColorKey.svelte frontend-school/src/routes/\(app\)/staff/calendar/+page.svelte frontend-school/tests/static/calendar.test.mjs
git commit -m "refactor(calendar): share color key"
```

### Task 3: Staff Public-Link Copy Action

**Files:**
- Modify: `frontend-school/tests/static/calendar.test.mjs`
- Modify: `frontend-school/src/routes/(app)/staff/calendar/+page.svelte`

**Interfaces:**
- Produces: `copyPublicCalendarLink(): Promise<void>`

- [ ] **Step 1: Write the failing clipboard-action test**

Add:

```js
test('staff calendar copies the current school public URL with feedback', async () => {
	const staffPage = await readProjectFile('src/routes/(app)/staff/calendar/+page.svelte');
	const copyBody = svelteFunctionBody(staffPage, 'copyPublicCalendarLink');

	assert.match(staffPage, /from '\$app\/state'/);
	assert.match(staffPage, /Copy/);
	assert.match(staffPage, /คัดลอกลิงก์สาธารณะ/);
	assert.match(copyBody, /page\.url\.origin/);
	assert.match(copyBody, /await navigator\.clipboard\.writeText/);
	assert.match(copyBody, /toast\.success\('คัดลอกลิงก์ปฏิทินสาธารณะแล้ว'\)/);
	assert.match(copyBody, /toast\.error\('คัดลอกลิงก์ไม่สำเร็จ'\)/);
});
```

- [ ] **Step 2: Run the static test and verify RED**

Run:

```bash
cd frontend-school
node --test tests/static/calendar.test.mjs
```

Expected: FAIL because `copyPublicCalendarLink` is missing.

- [ ] **Step 3: Implement the clipboard action**

Add imports:

```ts
import { page } from '$app/state';
import { Copy } from 'lucide-svelte';
```

Add:

```ts
async function copyPublicCalendarLink() {
	try {
		await navigator.clipboard.writeText(`${page.url.origin}/calendar`);
		toast.success('คัดลอกลิงก์ปฏิทินสาธารณะแล้ว');
	} catch {
		toast.error('คัดลอกลิงก์ไม่สำเร็จ');
	}
}
```

In the `PageShell` actions, before management-only buttons, add:

```svelte
{#if canReadCalendar}
	<Button variant="outline" onclick={copyPublicCalendarLink}>
		<Copy class="size-4" />
		คัดลอกลิงก์สาธารณะ
	</Button>
{/if}
```

- [ ] **Step 4: Format and verify GREEN**

Run:

```bash
cd frontend-school
npx prettier --write 'src/routes/(app)/staff/calendar/+page.svelte' tests/static/calendar.test.mjs
node --test tests/static/calendar.test.mjs
```

Expected: all calendar static tests pass.

- [ ] **Step 5: Commit the copy-action slice**

```bash
git add frontend-school/src/routes/\(app\)/staff/calendar/+page.svelte frontend-school/tests/static/calendar.test.mjs
git commit -m "feat(calendar): copy public calendar link"
```

### Task 4: Public Month Color Key

**Files:**
- Modify: `frontend-school/tests/static/calendar.test.mjs`
- Modify: `frontend-school/src/routes/(public)/calendar/+page.svelte`

**Interfaces:**
- Consumes:
  - `buildCalendarColorKey(selectedMonth, events)`
  - `<CalendarColorKey items={colorKeyItems} />`

- [ ] **Step 1: Write the failing public-page integration assertions**

Extend the public calendar test:

```js
assert.match(publicPage, /CalendarColorKey/);
assert.match(publicPage, /buildCalendarColorKey/);
assert.match(
	publicPage,
	/const colorKeyItems = \$derived\(buildCalendarColorKey\(selectedMonth, events\)\)/
);
assert.match(publicPage, /<CalendarColorKey items=\{colorKeyItems\} \/>/);
```

- [ ] **Step 2: Run the static test and verify RED**

Run:

```bash
cd frontend-school
node --test tests/static/calendar.test.mjs
```

Expected: FAIL because the public page does not use `CalendarColorKey`.

- [ ] **Step 3: Implement public-page derivation and rendering**

Import:

```ts
import CalendarColorKey from '$lib/components/calendar/CalendarColorKey.svelte';
import { buildCalendarColorKey } from '$lib/utils/calendar';
```

Add:

```ts
const colorKeyItems = $derived(buildCalendarColorKey(selectedMonth, events));
```

After the public header and before the loading/error/workspace block, add:

```svelte
{#if !loading && !error && colorKeyItems.length > 0}
	<CalendarColorKey items={colorKeyItems} />
{/if}
```

- [ ] **Step 4: Format and verify GREEN**

Run:

```bash
cd frontend-school
npx prettier --write 'src/routes/(public)/calendar/+page.svelte' tests/static/calendar.test.mjs
node --test tests/static/calendar.test.mjs tests/static/calendar-utils.test.mjs
```

Expected: all focused calendar tests pass.

- [ ] **Step 5: Commit the public-page slice**

```bash
git add frontend-school/src/routes/\(public\)/calendar/+page.svelte frontend-school/tests/static/calendar.test.mjs
git commit -m "feat(calendar): explain public event colors"
```

### Task 5: Svelte And Responsive Verification

**Files:**
- Verify all files changed by Tasks 1–4.

**Interfaces:**
- No new production interfaces.

- [ ] **Step 1: Run Svelte autofixer on every changed Svelte file**

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer src/lib/components/calendar/CalendarColorKey.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/calendar/+page.svelte' --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/routes/(public)/calendar/+page.svelte' --svelte-version 5
```

Expected for every file:

```json
{
  "issues": [],
  "suggestions": [],
  "require_another_tool_call_after_fixing": false
}
```

- [ ] **Step 2: Run type checks and all static tests**

```bash
cd frontend-school
npm run check
npm run test:static
```

Expected: Svelte reports 0 errors and 0 warnings; all static tests pass.

- [ ] **Step 3: Run the production build**

```bash
cd frontend-school
npm run build
```

Expected: Vite exits 0 and the Cloudflare adapter completes.

- [ ] **Step 4: Browser-check responsive behavior**

Start the local app with a tenant subdomain override, mock `/api/public/calendar/events`, and verify:

- 390×844 and 390×667 keep `document.documentElement.scrollHeight === window.innerHeight`.
- The color key is present, is a single horizontally scrollable row on mobile, and includes only selected-month categories.
- Clicking a mobile day still opens the day timeline dialog.
- 1440×900 keeps the desktop detail panel visible and the document height equal to the viewport.
- On the staff page, a mocked clipboard success receives the exact current-origin `/calendar` URL and a mocked rejection produces the error toast.

- [ ] **Step 5: Run repository hygiene checks**

```bash
git diff --check
git status -sb
```

Expected: no whitespace errors and only intentional commits/files from this feature.
