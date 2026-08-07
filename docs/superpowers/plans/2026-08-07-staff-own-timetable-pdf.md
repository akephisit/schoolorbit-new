# Staff Own Timetable PDF Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a direct, self-service PDF download for the signed-in teacher's currently selected timetable at `/staff/timetable`.

**Architecture:** Keep the existing `/api/me/timetable` data flow and build one instructor-mode `TimetablePage` from the page's loaded entries and derived periods. Reuse `generateTimetablePDF`, whose PDF dependency is loaded only after the action is clicked, and expose the action through the existing `PageShell` header pattern.

**Tech Stack:** SvelteKit 5, Svelte 5 runes, TypeScript, shadcn-svelte button primitives, lucide-svelte icons, pdfmake through the existing `generateTimetablePDF` utility, Node test runner.

## Global Constraints

- Download exactly the signed-in teacher's timetable already returned by `/api/me/timetable`; do not accept or add another teacher identifier.
- Produce one A4 landscape timetable per PDF page through the existing `full` layout.
- Match the timetable-management action with an outlined `ดาวน์โหลด PDF` button, download icon, and action-specific spinner.
- Disable download while timetable data is loading, while export is running, or when entries or periods are empty.
- Do not add backend endpoints, permissions, migrations, generated contract changes, or new PDF dependencies.
- Preserve the current page selectors, empty states, table display, and room-code fallback.

---

### Task 1: Add and verify the staff timetable PDF action

**Files:**
- Create: `frontend-school/tests/static/staff-own-timetable-pdf.test.mjs`
- Modify: `frontend-school/src/routes/(app)/staff/timetable/+page.svelte`

**Interfaces:**
- Consumes: `entries: TimetableEntry[]`, `periods: TimetablePeriodSummary[]`, `years: AcademicYear[]`, `semesters: Semester[]`, `selectedYearId`, `selectedSemesterId`, and `userName` already owned by the page.
- Consumes: `generateTimetablePDF(pages: TimetablePage[], fileName?: string, options?: { layout?: 'full' | 'portrait-2col' }): Promise<void>` from `$lib/utils/pdf`.
- Produces: `handleDownloadPDF(): Promise<void>` and `isExportingPdf: boolean` for the `PageShell` action.
- Produces: one instructor-mode PDF page whose period input is normalized to `{ id, order_index, name, start_time, end_time }`.

- [ ] **Step 1: Write the failing static regression test**

Create `frontend-school/tests/static/staff-own-timetable-pdf.test.mjs`:

```js
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '../..');

test('staff can download their loaded timetable as the shared PDF format', async () => {
	const page = await readFile(
		path.join(projectRoot, 'src/routes/(app)/staff/timetable/+page.svelte'),
		'utf8'
	);

	assert.match(page, /import \{ generateTimetablePDF \} from '\$lib\/utils\/pdf'/);
	assert.match(page, /let isExportingPdf = \$state\(false\)/);
	assert.match(page, /async function handleDownloadPDF\(\)/);

	const handler = page.slice(
		page.indexOf('async function handleDownloadPDF()'),
		page.indexOf('\n\t$effect', page.indexOf('async function handleDownloadPDF()'))
	);

	assert.match(handler, /timetableEntries: entries/);
	assert.match(handler, /viewMode: 'INSTRUCTOR'/);
	assert.match(handler, /periods\.map\(\(period, orderIndex\) =>/);
	assert.match(handler, /generateTimetablePDF\(/);
	assert.match(handler, /layout: 'full'/);
	assert.match(handler, /toast\.error\('ดาวน์โหลดตารางสอนไม่สำเร็จ'\)/);
	assert.match(handler, /finally[\s\S]*isExportingPdf = false/);

	assert.match(page, /\{#snippet actions\(\)\}/);
	assert.match(page, /onclick=\{handleDownloadPDF\}/);
	assert.match(page, /variant="outline"/);
	assert.match(page, /ดาวน์โหลด PDF/);
	assert.match(
		page,
		/disabled=\{loading \|\| isExportingPdf \|\| entries\.length === 0 \|\| periods\.length === 0\}/
	);
});
```

- [ ] **Step 2: Run the focused test and verify the red state**

Run from `frontend-school`:

```bash
node --test tests/static/staff-own-timetable-pdf.test.mjs
```

Expected: FAIL because the page does not yet import `generateTimetablePDF` or define `handleDownloadPDF`.

- [ ] **Step 3: Add imports and action-specific export state**

In `frontend-school/src/routes/(app)/staff/timetable/+page.svelte`, add the existing UI/export dependencies and extend the icon import:

```svelte
<script lang="ts">
	// existing imports remain
	import { Button } from '$lib/components/ui/button';
	import { generateTimetablePDF } from '$lib/utils/pdf';
	import { Download, Loader2, School, MapPin } from 'lucide-svelte';

	// existing page state remains
	let isExportingPdf = $state(false);
</script>
```

Replace the existing `{ School, MapPin }` lucide import rather than adding a duplicate import.

- [ ] **Step 4: Implement the direct full-layout PDF handler**

Add this handler before the existing `$effect` in the page script:

```ts
async function handleDownloadPDF() {
	if (loading || isExportingPdf || entries.length === 0 || periods.length === 0) return;

	const selectedYear = years.find((year) => year.id === selectedYearId);
	const selectedSemester = semesters.find((semester) => semester.id === selectedSemesterId);
	const teacherLabel = userName
		? userName.startsWith('ครู')
			? userName
			: `ครู${userName}`
		: 'ครู';
	const semesterLabel =
		selectedSemester?.name ||
		(selectedSemester?.term ? `ภาคเรียนที่ ${selectedSemester.term}` : 'ภาคเรียน');
	const academicYearLabel = selectedYear?.name || 'ปีการศึกษา';
	const subTitle = `${semesterLabel} ${academicYearLabel}`.trim();
	const fileName = `ตารางสอน ${teacherLabel} ${subTitle}`.replaceAll('/', '-');

	try {
		isExportingPdf = true;
		await generateTimetablePDF(
			[
				{
					title: `ตารางสอน ${teacherLabel}`,
					subTitle,
					periods: periods.map((period, orderIndex) => ({
						id: period.id,
						order_index: orderIndex,
						name: period.name,
						start_time: period.start_time ?? '',
						end_time: period.end_time ?? ''
					})),
					timetableEntries: entries,
					viewMode: 'INSTRUCTOR'
				}
			],
			fileName,
			{ layout: 'full' }
		);
		toast.success('ดาวน์โหลดตารางสอนแล้ว');
	} catch (error: unknown) {
		console.error('Failed to download timetable PDF', error);
		toast.error('ดาวน์โหลดตารางสอนไม่สำเร็จ');
	} finally {
		isExportingPdf = false;
	}
}
```

This uses only the already loaded self-service entries. `orderIndex` preserves the sorted `periodsFromTimetableEntries` order, and empty time fallbacks retain any period whose wire payload omitted its display times.

- [ ] **Step 5: Add the matching `PageShell` header action**

Place this named snippet immediately inside the existing `PageShell` before the selector surface:

```svelte
{#snippet actions()}
	<Button
		variant="outline"
		onclick={handleDownloadPDF}
		disabled={loading || isExportingPdf || entries.length === 0 || periods.length === 0}
	>
		{#if isExportingPdf}
			<Loader2 class="mr-2 h-4 w-4 animate-spin" />
		{:else}
			<Download class="mr-2 h-4 w-4" />
		{/if}
		ดาวน์โหลด PDF
	</Button>
{/snippet}
```

Do not add a layout-selection dialog: the approved behavior is an immediate one-teacher, one-table landscape download.

- [ ] **Step 6: Run the focused test and verify the green state**

Run from `frontend-school`:

```bash
node --test tests/static/staff-own-timetable-pdf.test.mjs
```

Expected: PASS with one passing subtest.

- [ ] **Step 7: Validate the changed Svelte component**

Run from `frontend-school`:

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/timetable/+page.svelte' --svelte-version 5
```

Expected: no issues. The pre-existing suggestion about calling `loadPeriodsAndEntries` from `$effect` may remain because that effect intentionally reacts to selected year/semester and authenticated-user readiness; this feature must not broaden scope into unrelated data-loading refactoring.

- [ ] **Step 8: Run the frontend verification matrix**

Run from `frontend-school`:

```bash
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:static
```

Expected: all commands exit successfully. If an existing unrelated failure appears, record the exact command and failure without weakening or skipping the required check.

- [ ] **Step 9: Review repository integrity and the final diff**

Run from the repository root:

```bash
git diff --check
git diff -- frontend-school/src/routes/'(app)'/staff/timetable/+page.svelte frontend-school/tests/static/staff-own-timetable-pdf.test.mjs
git status --short
```

Expected: `git diff --check` exits successfully; the diff contains only the approved self-service action and its focused test; status lists only those implementation files in addition to previously committed workflow artifacts.

- [ ] **Step 10: Commit the implementation**

```bash
git add frontend-school/src/routes/'(app)'/staff/timetable/+page.svelte frontend-school/tests/static/staff-own-timetable-pdf.test.mjs
git commit -m "feat: download own staff timetable PDF"
```

Expected: one implementation commit containing the page change and its focused regression test.
