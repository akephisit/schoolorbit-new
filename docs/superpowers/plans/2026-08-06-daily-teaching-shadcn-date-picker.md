# Daily Teaching shadcn Date Picker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Daily Teaching Today route's native date input with the shared shadcn-svelte DatePicker while preserving adjacent-day navigation and ISO date state.

**Architecture:** Keep `selectedDate` as the existing `YYYY-MM-DD` string and bind it directly to the local shared DatePicker. Add a focused route contract guard before changing the Svelte page, then validate the component with the Svelte analyzer and the full frontend matrix.

**Tech Stack:** SvelteKit 5, TypeScript, shadcn-svelte, Bits UI calendar/popover primitives, Tailwind CSS, Node test runner.

## Global Constraints

- Use `DatePicker` from `$lib/components/ui/date-picker`; do not compose a page-local calendar.
- Keep the previous-day and next-day icon buttons.
- Keep `selectedDate` as an ISO `YYYY-MM-DD` string with no additional state or conversion layer.
- The DatePicker trigger uses Thai abbreviated month and Buddhist year formatting owned by the shared component.
- Keep existing overview request keys, API parameters, effects, `moveDate`, and responsive filter layout unchanged.
- Do not change the shared DatePicker, backend, API contracts, permissions, migrations, realtime behavior, timetable content, or table widths.
- Execute on `main` only after explicit user authorization and do not push without explicit authorization.

---

### Task 1: Guard and replace the native teaching-date control

**Files:**
- Modify: `frontend-school/tests/static/api-global-contract.test.mjs:1153`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte:1-12,424-440`

**Interfaces:**
- Consumes: shared `DatePicker` props `id?: string`, bindable `value?: string`, `placeholder?: string`, and `class?: string`.
- Preserves: `selectedDate: string`, `moveDate(offsetDays: number)`, and the existing overview-loading effect.
- Produces: a shadcn-svelte date trigger bound to `selectedDate`, flanked by the existing arrow buttons.

- [ ] **Step 1: Add the failing route contract assertions**

Inside `test('daily teaching overview page is table based and read only', ...)`, add:

```js
assert.match(page, /from '\$lib\/components\/ui\/date-picker'/);
assert.match(
	page,
	/<DatePicker[\s\S]*id="teaching-date"[\s\S]*bind:value=\{selectedDate\}[\s\S]*placeholder="เลือกวันที่"[\s\S]*class="min-w-0 flex-1"/
);
assert.doesNotMatch(page, /<Input[\s\S]*id="teaching-date"[\s\S]*type="date"/);
```

Production mutation caught: replacing the shared DatePicker with a native teaching-date input fails the contract.

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
cd frontend-school
node --test tests/static/api-global-contract.test.mjs
```

Expected: exactly the Daily Teaching page test fails because the DatePicker import and markup are absent and the native input remains.

- [ ] **Step 3: Import the shared DatePicker**

Add this import beside the other local UI primitives:

```ts
import { DatePicker } from '$lib/components/ui/date-picker';
```

Keep the existing `Input` import because the same page still uses it for the teacher search field.

- [ ] **Step 4: Replace the native input and stabilize the flex row**

Keep both buttons and add `class="shrink-0"`. Replace only the native input with:

```svelte
<DatePicker
	id="teaching-date"
	bind:value={selectedDate}
	placeholder="เลือกวันที่"
	class="min-w-0 flex-1"
/>
```

The resulting row remains:

```svelte
<div class="flex gap-2">
	<Button
		variant="outline"
		size="icon"
		class="shrink-0"
		onclick={() => moveDate(-1)}
		aria-label="วันก่อนหน้า"
	>
		<ChevronLeft class="h-4 w-4" />
	</Button>
	<DatePicker
		id="teaching-date"
		bind:value={selectedDate}
		placeholder="เลือกวันที่"
		class="min-w-0 flex-1"
	/>
	<Button
		variant="outline"
		size="icon"
		class="shrink-0"
		onclick={() => moveDate(1)}
		aria-label="วันถัดไป"
	>
		<ChevronRight class="h-4 w-4" />
	</Button>
</div>
```

- [ ] **Step 5: Format and validate the Svelte component**

Run:

```bash
cd frontend-school
npx prettier --write \
  'src/routes/(app)/staff/academic/timetable/today/+page.svelte' \
  tests/static/api-global-contract.test.mjs
npx @sveltejs/mcp svelte-autofixer \
  'src/routes/(app)/staff/academic/timetable/today/+page.svelte' \
  --svelte-version 5
```

Expected: the Svelte autofixer reports no issues. Existing effect suggestions may remain because this task does not change state-loading architecture.

- [ ] **Step 6: Run the focused test and verify GREEN**

Run:

```bash
cd frontend-school
node --test tests/static/api-global-contract.test.mjs
```

Expected: all API global contract tests PASS with zero failures.

- [ ] **Step 7: Commit the tested component substitution**

```bash
git add \
  frontend-school/tests/static/api-global-contract.test.mjs \
  'frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte'
git commit -m "style: use shadcn date picker for daily teaching"
```

---

### Task 2: Full frontend verification and final review

**Files:**
- Review: `frontend-school/tests/static/api-global-contract.test.mjs`
- Review: `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte`

**Interfaces:**
- Consumes: the completed Task 1 commit.
- Produces: verification evidence and a clean committed `main` working tree.

- [ ] **Step 1: Run the frontend verification matrix**

From `frontend-school` run:

```bash
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:static
```

Expected: every command exits 0; Svelte reports 0 errors and 0 warnings; all test summaries report 0 failures.

- [ ] **Step 2: Review the final requirements**

Confirm directly in the final diff:

```text
The Today page imports DatePicker from the shared date-picker index.
The teaching-date control binds DatePicker to selectedDate.
The native teaching-date input is absent.
Both arrow buttons remain and use shrink-0.
The DatePicker uses min-w-0 flex-1.
The teacher-search Input remains available.
No shared DatePicker, backend, generated contract, permission, or migration file changed.
```

- [ ] **Step 3: Check repository hygiene**

From the repository root run:

```bash
git diff --check
git status --short --branch
git log -4 --oneline --decorate
```

Expected: no whitespace errors, no uncommitted implementation files, and the design, plan, and implementation commits are visible on `main`. Do not push without explicit user authorization.
