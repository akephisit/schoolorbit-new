# Calendar Color Key Placement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the visible calendar color-key heading from both calendar pages and place the public color key below its month grid without breaking the full-viewport responsive layout.

**Architecture:** Keep `CalendarColorKey.svelte` as the single shared renderer and remove its visible heading at the component boundary. Restructure only the public page's left calendar column so the month grid consumes remaining height and the color key sits beneath it; keep all event filtering and API behavior unchanged.

**Tech Stack:** SvelteKit 5, TypeScript, Tailwind CSS, Node.js static tests, Playwright browser validation

## Global Constraints

- The visible text `คำอธิบายสี` must not appear on either calendar page.
- The color-key container must keep the non-visual screen-reader label `หมวดหมู่กิจกรรมในปฏิทิน`.
- Public color-key data must remain limited to categories used by events overlapping the selected month.
- Desktop selected-day details and the mobile timeline dialog must remain unchanged.
- The public page must retain its full-viewport layout without document-level vertical scrolling.

---

### Task 1: Remove the Shared Visible Color-Key Heading

**Files:**
- Modify: `frontend-school/tests/static/calendar.test.mjs`
- Modify: `frontend-school/src/lib/components/calendar/CalendarColorKey.svelte`

**Interfaces:**
- Consumes: `CalendarColorKeyItem[]` through the existing `items` prop.
- Produces: the same horizontally scrollable category list, identified accessibly by `aria-label="หมวดหมู่กิจกรรมในปฏิทิน"`.

- [ ] **Step 1: Write the failing shared-component assertions**

Replace the existing color-key heading assertion in `calendar shared components use shadcn primitives` with:

```js
assert.match(colorKey, /CalendarColorKeyItem/);
assert.doesNotMatch(colorKey, /คำอธิบายสี/);
assert.match(colorKey, /aria-label="หมวดหมู่กิจกรรมในปฏิทิน"/);
assert.match(colorKey, /\{#each items as item \(item\.id\)\}/);
assert.match(colorKey, /overflow-x-auto/);
assert.match(colorKey, /sm:flex-wrap/);
```

- [ ] **Step 2: Run the focused static test and verify RED**

Run:

```bash
cd frontend-school
node --test tests/static/calendar.test.mjs
```

Expected: FAIL because `CalendarColorKey.svelte` still contains `คำอธิบายสี` and the old `aria-label`.

- [ ] **Step 3: Remove the heading while preserving responsive behavior**

Change `CalendarColorKey.svelte` to:

```svelte
<script lang="ts">
	import type { CalendarColorKeyItem } from '$lib/utils/calendar';

	let { items = [] }: { items?: CalendarColorKeyItem[] } = $props();
</script>

<div
	class="flex shrink-0 overflow-x-auto px-1 pb-1 text-xs text-muted-foreground sm:overflow-visible"
	aria-label="หมวดหมู่กิจกรรมในปฏิทิน"
>
	<div class="flex min-w-max items-center gap-4 sm:min-w-0 sm:flex-wrap">
		{#each items as item (item.id)}
			<span class="flex shrink-0 items-center gap-1.5 whitespace-nowrap">
				<span class="size-2 rounded-full" style:background-color={item.color} aria-hidden="true"
				></span>
				{item.name}
			</span>
		{/each}
	</div>
</div>
```

- [ ] **Step 4: Run the focused static test and verify GREEN**

Run:

```bash
cd frontend-school
node --test tests/static/calendar.test.mjs
```

Expected: all tests in `calendar.test.mjs` pass.

- [ ] **Step 5: Run the Svelte autofixer**

Run the official Svelte autofixer against `CalendarColorKey.svelte` and repeat until it reports zero issues and zero suggestions.

- [ ] **Step 6: Commit the shared component change**

```bash
git add frontend-school/tests/static/calendar.test.mjs frontend-school/src/lib/components/calendar/CalendarColorKey.svelte
git commit -m "refactor(calendar): simplify color key"
```

---

### Task 2: Move the Public Color Key Below the Month Grid

**Files:**
- Modify: `frontend-school/tests/static/calendar.test.mjs`
- Modify: `frontend-school/src/routes/(public)/calendar/+page.svelte`

**Interfaces:**
- Consumes: the existing `colorKeyItems` derived value and `CalendarMonthGrid` `fillHeight` prop.
- Produces: a left calendar column where the month grid occupies remaining height and `CalendarColorKey` follows it.

- [ ] **Step 1: Write the failing placement assertions**

Add these assertions to `public calendar fills mobile viewport and opens selected days in a timeline dialog`:

```js
const monthGridPosition = publicPage.indexOf('<CalendarMonthGrid');
const colorKeyPosition = publicPage.indexOf('<CalendarColorKey items={colorKeyItems} />');
const detailPanelPosition = publicPage.indexOf('<aside');

assert.ok(monthGridPosition >= 0, 'Expected the public month grid');
assert.ok(colorKeyPosition > monthGridPosition, 'Expected the public color key below the month grid');
assert.ok(detailPanelPosition > colorKeyPosition, 'Expected the color key in the left calendar column');
assert.match(publicPage, /class="flex min-h-0 min-w-0 flex-col gap-3"/);
assert.match(publicPage, /class="min-h-0 flex-1"[\s\S]*<CalendarMonthGrid/);
```

- [ ] **Step 2: Run the focused static test and verify RED**

Run:

```bash
cd frontend-school
node --test tests/static/calendar.test.mjs
```

Expected: FAIL because the current color key appears before the main grid and the left flex column does not exist.

- [ ] **Step 3: Restructure the public left calendar column**

Remove the color-key block above the loading/content state:

```svelte
{#if !loading && !error && colorKeyItems.length > 0}
	<CalendarColorKey items={colorKeyItems} />
{/if}
```

Inside the successful content grid, replace the direct `CalendarMonthGrid` with:

```svelte
<div class="flex min-h-0 min-w-0 flex-col gap-3">
	<div class="min-h-0 flex-1">
		<CalendarMonthGrid
			monthDate={selectedMonth}
			{events}
			{selectedDate}
			onselect={selectDate}
			fillHeight
		/>
	</div>
	{#if colorKeyItems.length > 0}
		<CalendarColorKey items={colorKeyItems} />
	{/if}
</div>
```

Keep the existing `<aside>` immediately after this new left-column wrapper.

- [ ] **Step 4: Run the focused static test and verify GREEN**

Run:

```bash
cd frontend-school
node --test tests/static/calendar.test.mjs
```

Expected: all tests in `calendar.test.mjs` pass.

- [ ] **Step 5: Run the Svelte autofixer**

Run the official Svelte autofixer against the public calendar page and repeat until it reports zero issues and zero suggestions.

- [ ] **Step 6: Commit the public layout change**

```bash
git add frontend-school/tests/static/calendar.test.mjs 'frontend-school/src/routes/(public)/calendar/+page.svelte'
git commit -m "refactor(calendar): place public color key below grid"
```

---

### Task 3: Verify Responsive Layout and Production Readiness

**Files:**
- Verify: `frontend-school/src/lib/components/calendar/CalendarColorKey.svelte`
- Verify: `frontend-school/src/routes/(public)/calendar/+page.svelte`
- Verify: `frontend-school/src/routes/(app)/staff/calendar/+page.svelte`

**Interfaces:**
- Consumes: the completed shared component and public layout changes.
- Produces: verification evidence only; no new runtime interface.

- [ ] **Step 1: Run focused calendar tests**

Run:

```bash
cd frontend-school
node --test tests/static/calendar.test.mjs tests/static/calendar-utils.test.mjs
```

Expected: 18 tests pass and 0 fail.

- [ ] **Step 2: Run Svelte diagnostics**

Run:

```bash
cd frontend-school
npm run check
```

Expected: `svelte-check found 0 errors and 0 warnings`.

- [ ] **Step 3: Build the production bundle**

Run:

```bash
cd frontend-school
npm run build
```

Expected: Vite and the Cloudflare adapter complete with exit code 0.

- [ ] **Step 4: Validate in a browser**

Start the local frontend:

```bash
cd frontend-school
PUBLIC_SCHOOL_SUBDOMAIN=snwsb npm run dev -- --host 127.0.0.1 --port 5173
```

Mock `**/api/public/calendar/events**` with selected-month categories, one adjacent-month-only category, and one uncategorized event. Validate at `1440x900`, `390x844`, and `390x667`:

- no visible `คำอธิบายสี`;
- category dots appear below the month grid;
- the adjacent-month-only category is absent;
- the mobile category list scrolls horizontally when needed;
- `document.documentElement.scrollHeight <= window.innerHeight + 1`;
- clicking a mobile date still opens the day timeline dialog.

- [ ] **Step 5: Check repository hygiene**

Run:

```bash
git diff --check
git status --short --branch
```

Expected: no whitespace errors and no uncommitted files from the implementation or browser harness.
