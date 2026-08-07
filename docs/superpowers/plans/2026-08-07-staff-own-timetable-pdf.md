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
- Create: `frontend-school/src/lib/utils/staff-own-timetable-pdf.ts`
- Create: `frontend-school/tests/static/staff-own-timetable-pdf.test.mjs`
- Modify: `frontend-school/src/routes/(app)/staff/timetable/+page.svelte`

**Interfaces:**
- Consumes: `entries: TimetableEntry[]`, `periods: TimetablePeriodSummary[]`, `years: AcademicYear[]`, `semesters: Semester[]`, `selectedYearId`, `selectedSemesterId`, and `userName` already owned by the page.
- Consumes: `generateTimetablePDF(pages: TimetablePage[], fileName?: string, options?: { layout?: 'full' | 'portrait-2col' }): Promise<void>` from `$lib/utils/pdf`.
- Produces: `buildStaffOwnTimetablePdfDownload(input): StaffOwnTimetablePdfDownload`, returning a filename and one instructor-mode PDF page whose period input is normalized to `{ id, order_index, name, start_time, end_time }`.
- Produces: `handleDownloadPDF(): Promise<void>` and `isExportingPdf: boolean` for the `PageShell` action.

- [ ] **Step 1: Write the failing behavior test**

Create `frontend-school/tests/static/staff-own-timetable-pdf.test.mjs`:

```js
import assert from 'node:assert/strict';
import test from 'node:test';

test('builds one instructor PDF page from the loaded self-service timetable', async () => {
	const module = await import('../../src/lib/utils/staff-own-timetable-pdf.ts').catch(() => ({}));
	assert.equal(typeof module.buildStaffOwnTimetablePdfDownload, 'function');

	const entries = [{ id: 'entry-1', room_code: 'MATH-1' }];
	const result = module.buildStaffOwnTimetablePdfDownload({
		teacherName: 'สายใจ / วิทยา',
		semesterName: '',
		semesterTerm: '1',
		academicYearName: 'ปีการศึกษา 2569',
		entries,
		periods: [
			{
				id: 'period-2',
				name: 'คาบ 2',
				start_time: '09:20:00',
				end_time: '10:10:00'
			},
			{ id: 'period-activity', name: 'กิจกรรม' }
		]
	});

	assert.equal(
		result.fileName,
		'ตารางสอน ครูสายใจ - วิทยา ภาคเรียนที่ 1 ปีการศึกษา 2569'
	);
	assert.deepEqual(result.pages, [
		{
			title: 'ตารางสอน ครูสายใจ / วิทยา',
			subTitle: 'ภาคเรียนที่ 1 ปีการศึกษา 2569',
			periods: [
				{
					id: 'period-2',
					order_index: 0,
					name: 'คาบ 2',
					start_time: '09:20:00',
					end_time: '10:10:00'
				},
				{
					id: 'period-activity',
					order_index: 1,
					name: 'กิจกรรม',
					start_time: '',
					end_time: ''
				}
			],
			timetableEntries: entries,
			viewMode: 'INSTRUCTOR'
		}
	]);
});
```

- [ ] **Step 2: Run the focused test and verify the red state**

Run from `frontend-school`:

```bash
node --test tests/static/staff-own-timetable-pdf.test.mjs
```

Expected: FAIL with `actual: 'undefined'` because `buildStaffOwnTimetablePdfDownload` does not exist yet.

- [ ] **Step 3: Implement the pure PDF download builder**

Create `frontend-school/src/lib/utils/staff-own-timetable-pdf.ts`:

```ts
import type { TimetableEntry, TimetablePeriodSummary } from '$lib/api/timetable';
import type { TimetablePage } from '$lib/utils/pdf';

export interface StaffOwnTimetablePdfInput {
	teacherName: string;
	semesterName?: string | null;
	semesterTerm?: string | null;
	academicYearName?: string | null;
	entries: TimetableEntry[];
	periods: TimetablePeriodSummary[];
}

export interface StaffOwnTimetablePdfDownload {
	fileName: string;
	pages: TimetablePage[];
}

export function buildStaffOwnTimetablePdfDownload(
	input: StaffOwnTimetablePdfInput
): StaffOwnTimetablePdfDownload {
	const normalizedTeacherName = input.teacherName.trim();
	const teacherLabel = normalizedTeacherName
		? normalizedTeacherName.startsWith('ครู')
			? normalizedTeacherName
			: `ครู${normalizedTeacherName}`
		: 'ครู';
	const semesterName = input.semesterName?.trim();
	const semesterTerm = input.semesterTerm?.trim();
	const semesterLabel = semesterName || (semesterTerm ? `ภาคเรียนที่ ${semesterTerm}` : 'ภาคเรียน');
	const academicYearLabel = input.academicYearName?.trim() || 'ปีการศึกษา';
	const subTitle = `${semesterLabel} ${academicYearLabel}`;
	const title = `ตารางสอน ${teacherLabel}`;

	return {
		fileName: `${title} ${subTitle}`.replaceAll('/', '-'),
		pages: [
			{
				title,
				subTitle,
				periods: input.periods.map((period, orderIndex) => ({
					id: period.id,
					order_index: orderIndex,
					name: period.name,
					start_time: period.start_time ?? '',
					end_time: period.end_time ?? ''
				})),
				timetableEntries: input.entries,
				viewMode: 'INSTRUCTOR'
			}
		]
	};
}
```

- [ ] **Step 4: Run the focused behavior test and verify the green state**

Run from `frontend-school`:

```bash
node --test tests/static/staff-own-timetable-pdf.test.mjs
```

Expected: PASS with one passing subtest. Mentally mutate teacher prefixing, slash replacement, period order, empty-time fallback, and instructor mode; the literal expectations must fail for each regression.

- [ ] **Step 5: Add imports and action-specific export state**

In `frontend-school/src/routes/(app)/staff/timetable/+page.svelte`, add the existing UI/export dependencies and extend the icon import:

```svelte
<script lang="ts">
	// existing imports remain
	import { Button } from '$lib/components/ui/button';
	import { generateTimetablePDF } from '$lib/utils/pdf';
	import { buildStaffOwnTimetablePdfDownload } from '$lib/utils/staff-own-timetable-pdf';
	import { Download, Loader2, School, MapPin } from 'lucide-svelte';

	// existing page state remains
	let isExportingPdf = $state(false);
</script>
```

Replace the existing `{ School, MapPin }` lucide import rather than adding a duplicate import.

- [ ] **Step 6: Implement the direct full-layout PDF handler**

Add this handler before the existing `$effect` in the page script:

```ts
async function handleDownloadPDF() {
	if (loading || isExportingPdf || entries.length === 0 || periods.length === 0) return;

	const selectedYear = years.find((year) => year.id === selectedYearId);
	const selectedSemester = semesters.find((semester) => semester.id === selectedSemesterId);
	const download = buildStaffOwnTimetablePdfDownload({
		teacherName: userName,
		semesterName: selectedSemester?.name,
		semesterTerm: selectedSemester?.term,
		academicYearName: selectedYear?.name,
		entries,
		periods
	});

	try {
		isExportingPdf = true;
		await generateTimetablePDF(download.pages, download.fileName, { layout: 'full' });
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

- [ ] **Step 7: Add the matching `PageShell` header action**

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

- [ ] **Step 8: Run the focused test and Svelte type check after integration**

Run from `frontend-school`:

```bash
node --test tests/static/staff-own-timetable-pdf.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: the focused behavior test and Svelte type check both pass.

- [ ] **Step 9: Validate the changed Svelte component**

Run from `frontend-school`:

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/timetable/+page.svelte' --svelte-version 5
```

Expected: no issues. The pre-existing suggestion about calling `loadPeriodsAndEntries` from `$effect` may remain because that effect intentionally reacts to selected year/semester and authenticated-user readiness; this feature must not broaden scope into unrelated data-loading refactoring.

- [ ] **Step 10: Run the frontend verification matrix**

Run from `frontend-school`:

```bash
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:static
```

Expected: all commands exit successfully. If an existing unrelated failure appears, record the exact command and failure without weakening or skipping the required check.

- [ ] **Step 11: Review repository integrity and the final diff**

Run from the repository root:

```bash
git diff --check
git diff -- frontend-school/src/lib/utils/staff-own-timetable-pdf.ts frontend-school/src/routes/'(app)'/staff/timetable/+page.svelte frontend-school/tests/static/staff-own-timetable-pdf.test.mjs
git status --short
```

Expected: `git diff --check` exits successfully; the diff contains only the approved self-service action and its focused test; status lists only those implementation files in addition to previously committed workflow artifacts.

- [ ] **Step 12: Commit the implementation**

```bash
git add frontend-school/src/lib/utils/staff-own-timetable-pdf.ts frontend-school/src/routes/'(app)'/staff/timetable/+page.svelte frontend-school/tests/static/staff-own-timetable-pdf.test.mjs
git commit -m "feat: download own staff timetable PDF"
```

Expected: one implementation commit containing the page change and its focused regression test.
