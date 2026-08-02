# WordPress Calendar Embed Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a dedicated public calendar embed and a staff-facing WordPress iframe-copy workflow that exposes only public SchoolOrbit events.

**Architecture:** Extract the existing public calendar UI and state into one mode-aware Svelte component shared by `/calendar` and `/calendar/embed`. Add a route-specific framing policy for the embed response, pure iframe URL/snippet helpers, and a focused staff dialog that previews and copies the current tenant's embed code.

**Tech Stack:** SvelteKit 2, Svelte 5 runes, TypeScript, Tailwind CSS, shadcn-svelte/bits-ui primitives, Node test runner, existing typed calendar API client.

## Global Constraints

- The embed must use only `CalendarPublicEvent` from `/api/public/calendar/events`; no authenticated endpoint or frontend-only privacy filter may be used.
- The public full-page route remains `/calendar`; the dedicated iframe route is `/calendar/embed`.
- The embed response uses `Content-Security-Policy: frame-ancestors 'self' https:` and must not add `X-Frame-Options`.
- The iframe snippet uses the current frontend origin, `width="100%"`, `height="760"`, lazy loading, `sandbox="allow-scripts allow-same-origin"`, and no cross-origin resize script.
- The staff dialog is available with calendar read access; management permission is not required.
- Keep the existing public-link copy action and the current `/calendar` user experience.
- Do not change the database, migrations, permissions, generated permission registries, Rust DTOs, OpenAPI contract, or generated API client.
- Use Svelte 5 runes and existing local UI primitives; run the Svelte autofixer on every touched `.svelte` file.

## File Map

- Create `frontend-school/src/lib/components/calendar/PublicCalendarView.svelte`: owns public calendar loading, navigation, selected-day behavior, and page/embed presentation.
- Create `frontend-school/src/lib/components/calendar/CalendarEmbedDialog.svelte`: owns WordPress instructions, preview, fallback code field, and copy feedback.
- Create `frontend-school/src/routes/(public)/calendar/embed/+page.svelte`: renders the shared view in embed mode and owns the browser title.
- Create `frontend-school/src/routes/(public)/calendar/embed/+page.server.ts`: sets the route-specific frame-ancestor policy.
- Modify `frontend-school/src/routes/(public)/calendar/+page.svelte`: reduce the route to its title and shared page-mode view.
- Modify `frontend-school/src/routes/(app)/staff/calendar/+page.svelte`: add the embed trigger, dialog state, and dialog instance.
- Modify `frontend-school/src/lib/utils/calendar.ts`: add pure embed URL and iframe snippet builders.
- Modify `frontend-school/tests/static/calendar-utils.test.mjs`: test the generated embed URL and exact safe iframe attributes.
- Modify `frontend-school/tests/static/calendar.test.mjs`: cover shared public behavior, embed route/policy, and staff dialog wiring.

---

### Task 1: Calendar Embed URL and Snippet Helpers

**Files:**
- Modify: `frontend-school/src/lib/utils/calendar.ts`
- Test: `frontend-school/tests/static/calendar-utils.test.mjs`

**Interfaces:**
- Produces: `buildCalendarEmbedUrl(origin: string): string`
- Produces: `buildCalendarEmbedCode(origin: string): string`
- Consumes: the runtime value `page.url.origin`, passed later by the staff dialog.

- [ ] **Step 1: Write the failing helper tests**

Add both helpers to the existing import in `calendar-utils.test.mjs`, then add:

```js
it('builds a tenant-local calendar embed URL', () => {
	assert.equal(
		buildCalendarEmbedUrl('https://snwsb.schoolorbit.app'),
		'https://snwsb.schoolorbit.app/calendar/embed'
	);
	assert.equal(
		buildCalendarEmbedUrl('https://snwsb.schoolorbit.app/'),
		'https://snwsb.schoolorbit.app/calendar/embed'
	);
});

it('builds a WordPress-safe calendar iframe snippet', () => {
	const code = buildCalendarEmbedCode('https://snwsb.schoolorbit.app');

	assert.match(code, /src="https:\/\/snwsb\.schoolorbit\.app\/calendar\/embed"/);
	assert.match(code, /title="ปฏิทินโรงเรียน"/);
	assert.match(code, /width="100%"/);
	assert.match(code, /height="760"/);
	assert.match(code, /loading="lazy"/);
	assert.match(code, /sandbox="allow-scripts allow-same-origin"/);
	assert.match(code, /referrerpolicy="strict-origin-when-cross-origin"/);
	assert.match(code, /style="border:0;border-radius:12px"/);
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:

```bash
cd frontend-school
node --test tests/static/calendar-utils.test.mjs
```

Expected: FAIL because `buildCalendarEmbedUrl` and `buildCalendarEmbedCode` are not exported.

- [ ] **Step 3: Implement the pure helpers**

Add near the existing calendar constants in `calendar.ts`:

```ts
export const CALENDAR_EMBED_HEIGHT = 760;

export function buildCalendarEmbedUrl(origin: string): string {
	return new URL('/calendar/embed', origin).toString();
}

export function buildCalendarEmbedCode(origin: string): string {
	return `<iframe
  src="${buildCalendarEmbedUrl(origin)}"
  title="ปฏิทินโรงเรียน"
  width="100%"
  height="${CALENDAR_EMBED_HEIGHT}"
  loading="lazy"
  sandbox="allow-scripts allow-same-origin"
  referrerpolicy="strict-origin-when-cross-origin"
  style="border:0;border-radius:12px"
></iframe>`;
}
```

- [ ] **Step 4: Run the focused helper tests**

Run:

```bash
node --test tests/static/calendar-utils.test.mjs
```

Expected: PASS for all calendar helper tests.

- [ ] **Step 5: Commit the helper contract**

```bash
git add src/lib/utils/calendar.ts tests/static/calendar-utils.test.mjs
git commit -m "feat: add calendar embed snippet helpers"
```

---

### Task 2: Shared Public Calendar View

**Files:**
- Create: `frontend-school/src/lib/components/calendar/PublicCalendarView.svelte`
- Modify: `frontend-school/src/routes/(public)/calendar/+page.svelte`
- Test: `frontend-school/tests/static/calendar.test.mjs`

**Interfaces:**
- Produces: Svelte component prop `mode?: 'page' | 'embed'`, defaulting to `page`.
- Consumes: `CalendarPublicEvent`, `listPublicCalendarEvents`, existing calendar utilities, and existing calendar display components.
- Preserves: the `/calendar` route's `data.title` browser title and full-page appearance.

- [ ] **Step 1: Rewrite the public-calendar static assertions to target the shared component**

In `calendar.test.mjs`, change the public section of `calendar read-only pages sort selected-day events consistently` to read `src/lib/components/calendar/PublicCalendarView.svelte`. Replace the final public-layout test with assertions shaped as follows:

```js
test('public calendar route delegates to the shared page-mode view', async () => {
	const publicPage = await readProjectFile('src/routes/(public)/calendar/+page.svelte');
	const publicView = await readProjectFile(
		'src/lib/components/calendar/PublicCalendarView.svelte'
	);

	assert.match(publicPage, /PublicCalendarView/);
	assert.match(publicPage, /mode="page"/);
	assert.match(publicPage, /<title>\{data\.title\}<\/title>/);
	assert.doesNotMatch(publicPage, /listPublicCalendarEvents/);
	assert.match(publicView, /type PublicCalendarMode = 'page' \| 'embed'/);
	assert.match(publicView, /listPublicCalendarEvents/);
	assert.match(publicView, /CalendarPublicEvent/);
	assert.match(publicView, /Number\(right\.allDay\) - Number\(left\.allDay\)/);
	assert.match(publicView, /window\.matchMedia\('\(max-width: 1023px\)'\)/);
	assert.match(publicView, /CalendarDayTimelineDialog/);
	assert.match(publicView, /CalendarColorKey/);
	assert.match(publicView, /CalendarMonthGrid/);
	assert.match(publicView, /CalendarEventList/);
	assert.match(publicView, /mode === 'embed'/);
});
```

Keep the timeline-dialog primitive assertions in the same test file, but read layout and selected-event behavior from `PublicCalendarView.svelte` instead of the route file.

- [ ] **Step 2: Run the focused static test to verify it fails**

Run:

```bash
node --test tests/static/calendar.test.mjs
```

Expected: FAIL with `ENOENT` for `PublicCalendarView.svelte`.

- [ ] **Step 3: Extract the public calendar into a mode-aware component**

Create `PublicCalendarView.svelte` with the current public-calendar behavior and an explicit presentation prop:

```svelte
<script lang="ts">
	import { onMount } from 'svelte';
	import { addMonths } from 'date-fns';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import CalendarColorKey from '$lib/components/calendar/CalendarColorKey.svelte';
	import CalendarDayTimelineDialog from '$lib/components/calendar/CalendarDayTimelineDialog.svelte';
	import CalendarMonthGrid from '$lib/components/calendar/CalendarMonthGrid.svelte';
	import CalendarEventList from '$lib/components/calendar/CalendarEventList.svelte';
	import { type CalendarPublicEvent, listPublicCalendarEvents } from '$lib/api/calendar';
	import {
		buildCalendarColorKey,
		calendarGridRange,
		eventOverlapsDate,
		formatCalendarDate,
		formatCalendarMonth,
		monthRange,
		toIsoDate
	} from '$lib/utils/calendar';
	import { CalendarDays, ChevronLeft, ChevronRight } from 'lucide-svelte';

	type PublicCalendarMode = 'page' | 'embed';

	let { mode = 'page' }: { mode?: PublicCalendarMode } = $props();
	const embedded = $derived(mode === 'embed');

	let events = $state.raw<CalendarPublicEvent[]>([]);
	let loading = $state(true);
	let error = $state('');
	let selectedMonth = $state(toIsoDate(new Date()));
	let selectedDate = $state(toIsoDate(new Date()));
	let dayDialogOpen = $state(false);

	const monthLabel = $derived(formatCalendarMonth(selectedMonth));
	const colorKeyItems = $derived(buildCalendarColorKey(selectedMonth, events));
	const selectedDateEvents = $derived(
		events
			.filter((event) => eventOverlapsDate(event, selectedDate))
			.sort(
				(left, right) =>
					left.startDate.localeCompare(right.startDate) ||
					Number(right.allDay) - Number(left.allDay) ||
					(left.startTime ?? '').localeCompare(right.startTime ?? '') ||
					left.title.localeCompare(right.title, 'th')
			)
	);

	async function loadCalendar() {
		loading = true;
		error = '';
		try {
			events = await listPublicCalendarEvents({ ...calendarGridRange(selectedMonth) });
		} catch (loadError: unknown) {
			error =
				(loadError instanceof Error ? loadError.message : String(loadError)) ||
				'โหลดปฏิทินไม่สำเร็จ';
		} finally {
			loading = false;
		}
	}

	async function changeMonth(offset: number) {
		const currentMonthStart = monthRange(selectedMonth).from;
		const nextMonth = monthRange(
			toIsoDate(addMonths(new Date(`${currentMonthStart}T00:00:00`), offset))
		).from;
		selectedMonth = nextMonth;
		selectedDate = nextMonth;
		await loadCalendar();
	}

	async function goToToday() {
		const today = toIsoDate(new Date());
		selectedMonth = monthRange(today).from;
		selectedDate = today;
		await loadCalendar();
	}

	function selectDate(date: string) {
		selectedDate = date;
		if (window.matchMedia('(max-width: 1023px)').matches) {
			dayDialogOpen = true;
		}
	}

	onMount(() => {
		void loadCalendar();
	});
</script>

<main class={embedded ? 'h-dvh overflow-hidden bg-background' : 'h-dvh overflow-hidden bg-muted/20'}>
	<section
		data-calendar-mode={mode}
		class={embedded
			? 'flex h-full w-full flex-col gap-2 p-2 sm:gap-3 sm:p-3'
			: 'mx-auto flex h-full w-full max-w-screen-2xl flex-col gap-3 px-3 py-3 sm:px-4 lg:gap-4 lg:px-8 lg:py-4 2xl:px-10'}
	>
		<header
			class={embedded
				? 'flex shrink-0 items-center justify-between gap-2 border-b pb-2'
				: 'flex shrink-0 flex-col gap-2 border-b pb-3 sm:flex-row sm:items-end sm:justify-between'}
		>
			{#if !embedded}
				<div class="flex items-center gap-3">
					<div
						class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary sm:size-10"
					>
						<CalendarDays class="size-5" />
					</div>
					<div class="min-w-0">
						<h1 class="text-lg font-semibold tracking-tight sm:text-2xl">ปฏิทินโรงเรียน</h1>
						<p class="hidden text-sm text-muted-foreground sm:block">
							กิจกรรมที่โรงเรียนเปิดเผยต่อสาธารณะ
						</p>
					</div>
				</div>
			{/if}

			<div
				class={embedded
					? 'flex w-full flex-wrap items-center justify-between gap-2'
					: 'flex flex-wrap items-center justify-between gap-2 sm:justify-end'}
			>
				<Button variant="outline" size="sm" onclick={goToToday}>วันนี้</Button>
				<div class="flex items-center gap-1 sm:gap-2">
					<Button
						variant="outline"
						size="icon-sm"
						onclick={() => changeMonth(-1)}
						aria-label="เดือนก่อนหน้า"
					>
						<ChevronLeft class="h-4 w-4" />
					</Button>
					<div class="min-w-32 text-center text-sm font-semibold sm:min-w-40">{monthLabel}</div>
					<Button
						variant="outline"
						size="icon-sm"
						onclick={() => changeMonth(1)}
						aria-label="เดือนถัดไป"
					>
						<ChevronRight class="h-4 w-4" />
					</Button>
				</div>
			</div>
		</header>

		{#if loading}
			<div class="min-h-0 flex-1 overflow-hidden">
				<PageSkeleton variant="detail" />
			</div>
		{:else if error}
			<div class="min-h-0 flex-1 overflow-y-auto">
				<PageState
					variant="error"
					title="โหลดปฏิทินไม่สำเร็จ"
					description={error}
					actionLabel="ลองอีกครั้ง"
					onaction={loadCalendar}
				/>
			</div>
		{:else}
			<div
				class="grid min-h-0 flex-1 lg:grid-cols-[minmax(0,1fr)_22rem] lg:gap-5 xl:grid-cols-[minmax(0,1fr)_24rem]"
			>
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
				<aside
					class="hidden min-h-0 flex-col overflow-hidden rounded-xl border bg-card shadow-sm lg:flex"
				>
					<div class="flex shrink-0 items-end justify-between gap-3 border-b px-3 py-2.5 sm:px-4">
						<div>
							<p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
								วันที่เลือก
							</p>
							<h2 class="mt-1 text-lg font-semibold">{formatCalendarDate(selectedDate)}</h2>
						</div>
						<span class="shrink-0 text-sm text-muted-foreground">
							{selectedDateEvents.length} รายการ
						</span>
					</div>
					<div class="min-h-0 flex-1 overflow-y-auto p-3 sm:p-4">
						<CalendarEventList events={selectedDateEvents} canManage={false} showFullDescription />
					</div>
				</aside>
			</div>
		{/if}
	</section>
</main>

<CalendarDayTimelineDialog
	bind:open={dayDialogOpen}
	date={selectedDate}
	events={selectedDateEvents}
/>
```

Do not add a second API path or management props. Both modes call only `listPublicCalendarEvents({ ...calendarGridRange(selectedMonth) })`.

- [ ] **Step 4: Reduce the existing route to title ownership and composition**

Replace `src/routes/(public)/calendar/+page.svelte` with:

```svelte
<script lang="ts">
	import PublicCalendarView from '$lib/components/calendar/PublicCalendarView.svelte';

	let { data } = $props();
</script>

<svelte:head>
	<title>{data.title}</title>
</svelte:head>

<PublicCalendarView mode="page" />
```

- [ ] **Step 5: Validate Svelte and run the focused static test**

Run:

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/calendar/PublicCalendarView.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/routes/(public)/calendar/+page.svelte' --svelte-version 5
node --test tests/static/calendar.test.mjs
```

Expected: both autofixer calls report no issues requiring another call, and the calendar static test passes.

- [ ] **Step 6: Commit the shared view**

```bash
git add src/lib/components/calendar/PublicCalendarView.svelte 'src/routes/(public)/calendar/+page.svelte' tests/static/calendar.test.mjs
git commit -m "refactor: share public calendar view"
```

---

### Task 3: Dedicated Embed Route and Framing Policy

**Files:**
- Create: `frontend-school/src/routes/(public)/calendar/embed/+page.svelte`
- Create: `frontend-school/src/routes/(public)/calendar/embed/+page.server.ts`
- Modify: `frontend-school/tests/static/calendar.test.mjs`

**Interfaces:**
- Consumes: `PublicCalendarView` with `mode="embed"` from Task 2.
- Produces: unauthenticated route `/calendar/embed`.
- Produces: response header `content-security-policy: frame-ancestors 'self' https:`.

- [ ] **Step 1: Add the failing embed-route contract test**

Append to `calendar.test.mjs`:

```js
test('calendar embed route is public, compact, and explicitly frameable', async () => {
	const embedPage = await readProjectFile('src/routes/(public)/calendar/embed/+page.svelte');
	const embedServer = await readProjectFile(
		'src/routes/(public)/calendar/embed/+page.server.ts'
	);

	assert.match(embedPage, /PublicCalendarView/);
	assert.match(embedPage, /mode="embed"/);
	assert.match(embedPage, /<title>\{data\.title\}<\/title>/);
	assert.doesNotMatch(embedPage, /listCalendarEvents|listMyCalendarEvents|listChildCalendarEvents/);
	assert.match(embedServer, /setHeaders/);
	assert.match(embedServer, /content-security-policy/);
	assert.match(embedServer, /frame-ancestors 'self' https:/);
	assert.doesNotMatch(embedServer, /X-Frame-Options|x-frame-options/);
});
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```bash
node --test tests/static/calendar.test.mjs
```

Expected: FAIL with `ENOENT` for the new embed route.

- [ ] **Step 3: Add the server load and response policy**

Create `+page.server.ts`:

```ts
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = ({ setHeaders }) => {
	setHeaders({
		'content-security-policy': "frame-ancestors 'self' https:"
	});

	return { title: 'ปฏิทินโรงเรียน' };
};
```

- [ ] **Step 4: Add the compact embed page**

Create `+page.svelte`:

```svelte
<script lang="ts">
	import PublicCalendarView from '$lib/components/calendar/PublicCalendarView.svelte';

	let { data } = $props();
</script>

<svelte:head>
	<title>{data.title}</title>
</svelte:head>

<PublicCalendarView mode="embed" />
```

- [ ] **Step 5: Validate the route and focused tests**

Run:

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(public)/calendar/embed/+page.svelte' --svelte-version 5
node --test tests/static/calendar.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: autofixer reports no remaining issue, calendar static tests pass, and Svelte check exits successfully.

- [ ] **Step 6: Commit the embed route**

```bash
git add 'src/routes/(public)/calendar/embed/+page.svelte' 'src/routes/(public)/calendar/embed/+page.server.ts' tests/static/calendar.test.mjs
git commit -m "feat: add public calendar embed route"
```

---

### Task 4: Staff WordPress Embed Dialog

**Files:**
- Create: `frontend-school/src/lib/components/calendar/CalendarEmbedDialog.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/calendar/+page.svelte`
- Modify: `frontend-school/tests/static/calendar.test.mjs`

**Interfaces:**
- Consumes: `buildCalendarEmbedUrl(origin)` and `buildCalendarEmbedCode(origin)` from Task 1.
- Component props: `open: boolean` as a bindable prop and `origin: string` as the current tenant frontend origin.
- Produces: staff action label `ฝังในเว็บไซต์`, a live iframe preview, visible readonly code, and clipboard feedback.

- [ ] **Step 1: Add the failing staff embed workflow test**

Append to `calendar.test.mjs`:

```js
test('staff calendar provides a WordPress embed dialog with manual copy fallback', async () => {
	const staffPage = await readProjectFile('src/routes/(app)/staff/calendar/+page.svelte');
	const embedDialog = await readProjectFile(
		'src/lib/components/calendar/CalendarEmbedDialog.svelte'
	);

	assert.match(staffPage, /CalendarEmbedDialog/);
	assert.match(staffPage, /let embedDialogOpen = \$state\(false\)/);
	assert.match(staffPage, /ฝังในเว็บไซต์/);
	assert.match(staffPage, /embedDialogOpen = true/);
	assert.match(staffPage, /bind:open=\{embedDialogOpen\}/);
	assert.match(staffPage, /origin=\{page\.url\.origin\}/);
	assert.match(embedDialog, /buildCalendarEmbedUrl/);
	assert.match(embedDialog, /buildCalendarEmbedCode/);
	assert.match(embedDialog, /บล็อก Custom HTML/);
	assert.match(embedDialog, /<iframe/);
	assert.match(embedDialog, /src=\{embedUrl\}/);
	assert.match(embedDialog, /readonly/);
	assert.match(embedDialog, /await navigator\.clipboard\.writeText\(embedCode\)/);
	assert.match(embedDialog, /คัดลอกโค้ดแล้ว/);
	assert.match(embedDialog, /เลือกและคัดลอกโค้ดด้านล่างด้วยตนเอง/);
});
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```bash
node --test tests/static/calendar.test.mjs
```

Expected: FAIL with `ENOENT` for `CalendarEmbedDialog.svelte`.

- [ ] **Step 3: Implement the focused dialog component**

Create `CalendarEmbedDialog.svelte` with this state and behavior:

```svelte
<script lang="ts">
	import { toast } from 'svelte-sonner';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { Textarea } from '$lib/components/ui/textarea';
	import { buildCalendarEmbedCode, buildCalendarEmbedUrl } from '$lib/utils/calendar';
	import { Copy } from 'lucide-svelte';

	let {
		open = $bindable(false),
		origin
	}: {
		open: boolean;
		origin: string;
	} = $props();

	const embedUrl = $derived(buildCalendarEmbedUrl(origin));
	const embedCode = $derived(buildCalendarEmbedCode(origin));

	async function copyEmbedCode() {
		try {
			await navigator.clipboard.writeText(embedCode);
			toast.success('คัดลอกโค้ดแล้ว');
		} catch {
			toast.error('คัดลอกไม่สำเร็จ เลือกและคัดลอกโค้ดด้านล่างด้วยตนเอง');
		}
	}
</script>
```

Use the existing dialog primitives and keep the fallback code visible at all times:

```svelte
<Dialog.Root bind:open>
	<Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-4xl">
		<Dialog.Header>
			<Dialog.Title>ฝังปฏิทินในเว็บไซต์</Dialog.Title>
			<Dialog.Description>
				เพิ่มบล็อก Custom HTML ใน WordPress แล้ววางโค้ดด้านล่าง
			</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4">
			<div class="overflow-hidden rounded-xl border bg-muted/20">
				<iframe
					src={embedUrl}
					title="ตัวอย่างปฏิทินโรงเรียน"
					class="h-[28rem] w-full border-0"
					loading="lazy"
					sandbox="allow-scripts allow-same-origin"
					referrerpolicy="strict-origin-when-cross-origin"
				></iframe>
			</div>

			<Textarea value={embedCode} readonly rows={9} class="font-mono text-xs" />
		</div>

		<Dialog.Footer>
			<Button type="button" variant="outline" onclick={() => (open = false)}>ปิด</Button>
			<Button type="button" onclick={copyEmbedCode}>
				<Copy class="size-4" />
				คัดลอกโค้ด
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
```

- [ ] **Step 4: Wire the dialog into the staff calendar**

In `src/routes/(app)/staff/calendar/+page.svelte`:

1. Import `CalendarEmbedDialog.svelte` and the `Code2` icon.
2. Add `let embedDialogOpen = $state(false);` beside the other dialog state.
3. Inside the existing `canReadCalendar` action block, add:

```svelte
<Button variant="outline" onclick={() => (embedDialogOpen = true)}>
	<Code2 class="size-4" />
	ฝังในเว็บไซต์
</Button>
```

4. Render the dialog next to the existing calendar dialogs:

```svelte
<CalendarEmbedDialog bind:open={embedDialogOpen} origin={page.url.origin} />
```

Do not move this action into `canManageCalendar`; read permission is the approved access boundary.

- [ ] **Step 5: Validate Svelte and focused tests**

Run:

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/calendar/CalendarEmbedDialog.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/calendar/+page.svelte' --svelte-version 5
node --test tests/static/calendar.test.mjs
node --test tests/static/calendar-utils.test.mjs
```

Expected: both autofixer calls report no remaining issue and both focused test files pass.

- [ ] **Step 6: Commit the staff workflow**

```bash
git add src/lib/components/calendar/CalendarEmbedDialog.svelte 'src/routes/(app)/staff/calendar/+page.svelte' tests/static/calendar.test.mjs
git commit -m "feat: add WordPress calendar embed workflow"
```

---

### Task 5: Full Frontend Verification and Diff Review

**Files:**
- Review: all files listed in the File Map.

**Interfaces:**
- Verifies: helper contract, shared component boundary, public embed route, frame policy, staff dialog, and unchanged generated contracts.
- Produces: evidence required by `.rules` before completion.

- [ ] **Step 1: Run the Svelte autofixer on every touched Svelte file**

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer src/lib/components/calendar/PublicCalendarView.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer src/lib/components/calendar/CalendarEmbedDialog.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/routes/(public)/calendar/+page.svelte' --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/routes/(public)/calendar/embed/+page.svelte' --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/calendar/+page.svelte' --svelte-version 5
```

Expected: every invocation reports `issues: []`, `suggestions: []`, and `require_another_tool_call_after_fixing: false`. Apply any required corrections and rerun the affected file until clean.

- [ ] **Step 2: Run focused and complete frontend checks**

```bash
node --test tests/static/calendar-utils.test.mjs tests/static/calendar.test.mjs
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

Expected: all commands exit with status 0.

- [ ] **Step 3: Confirm contracts and protected layers remain untouched**

Run from the repository root:

```bash
git diff --name-only 1a034ca5..HEAD
```

Expected: only the frontend source/test files in the File Map plus `docs/superpowers/plans/2026-08-02-wordpress-calendar-embed.md`; no migration, permission contract, generated permission registry, OpenAPI, generated API DTO, backend, or proxy file.

- [ ] **Step 4: Review whitespace, final diff, and worktree state**

```bash
git diff --check 1a034ca5..HEAD
git diff --stat 1a034ca5..HEAD
git status --short
```

Expected: no whitespace errors and no uncommitted implementation files. Inspect the complete diff before reporting completion.
