<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { addMonths } from 'date-fns';
	import { PageSkeleton, PageState, RegionUpdatingState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import CalendarSearchDialog from '#lib/components/calendar/CalendarSearchDialog.svelte';
	import CalendarColorKey from '#lib/components/calendar/CalendarColorKey.svelte';
	import CalendarDayTimelineDialog from '#lib/components/calendar/CalendarDayTimelineDialog.svelte';
	import CalendarMonthGrid from '#lib/components/calendar/CalendarMonthGrid.svelte';
	import CalendarEventList from '#lib/components/calendar/CalendarEventList.svelte';
	import { type CalendarPublicEvent, listPublicCalendarEvents } from '#lib/api/calendar.js';
	import {
		listPublicAcademicContextOptions,
		type AcademicYearOption
	} from '#lib/api/academic-context.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import type { PublicCalendarRouteData } from '#lib/calendar/public-route.js';
	import { calendarRouteFilters } from '#lib/utils/calendar-route-filters.js';
	import {
		publicCalendarRange,
		buildCalendarColorKey,
		eventOverlapsDate,
		formatCalendarDate,
		formatCalendarMonth,
		monthRange,
		toIsoDate
	} from '#lib/utils/calendar.js';
	import { CalendarDays, ChevronLeft, ChevronRight, Search } from '@lucide/svelte';

	type PublicCalendarMode = 'page' | 'embed';
	let { mode = 'page', data }: { mode?: PublicCalendarMode; data: PublicCalendarRouteData } =
		$props();
	const embedded = $derived(mode === 'embed');
	const currentUrl = $derived(
		page.state.publicCalendarUrl ? new URL(page.state.publicCalendarUrl) : new URL(page.url.href)
	);
	const selectedMonth = $derived(calendarRouteFilters(currentUrl).month);
	const selectedYearId = $derived(currentUrl.searchParams.get('academicYearId')?.trim() ?? '');
	const key = $derived(`${selectedMonth}|${selectedYearId}`);
	let events = $state.raw<CalendarPublicEvent[]>([]),
		years = $state<AcademicYearOption[]>([]);
	let loading = $state(true),
		rendered = $state(false),
		error = $state(''),
		yearError = $state(''),
		yearsLoading = $state(true);
	let selectedDate = $state(
			untrack(() => {
				const today = new Intl.DateTimeFormat('en-CA', {
					timeZone: 'Asia/Bangkok',
					year: 'numeric',
					month: '2-digit',
					day: '2-digit'
				}).format(new Date());
				return today.startsWith(data.month.slice(0, 7)) ? today : data.month;
			})
		),
		dayDialogOpen = $state(false),
		searchOpen = $state(false);
	let owner = '',
		jumpDate = '',
		consumed: typeof data.events | null = null;
	const request = new LatestRequest(),
		yearRequest = new LatestRequest();
	const selectedYear = $derived(years.find((year) => year.id === selectedYearId));
	const monthLabel = $derived(formatCalendarMonth(selectedMonth));
	const colorKeyItems = $derived(buildCalendarColorKey(selectedMonth, events));
	const canPrevious = $derived(
		!selectedYear || selectedMonth.slice(0, 7) > selectedYear.startDate.slice(0, 7)
	);
	const canNext = $derived(
		!selectedYear || selectedMonth.slice(0, 7) < selectedYear.endDate.slice(0, 7)
	);
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

	function applyEvents(result: Awaited<typeof data.events>, revision: number) {
		if (!request.isCurrent(revision)) return;
		loading = false;
		if (result.ok) {
			events = result.data;
			rendered = true;
			error = '';
			if (jumpDate && jumpDate === selectedDate) {
				selectDate(jumpDate);
				jumpDate = '';
			}
		} else error = result.error;
	}
	$effect.pre(() => {
		const source = data.years;
		untrack(() => {
			const ticket = yearRequest.begin();
			yearsLoading = true;
			yearError = '';
			void source.then((result) => {
				if (!yearRequest.isCurrent(ticket.revision)) return;
				yearsLoading = false;
				if (result.ok) years = result.data;
				else yearError = result.error;
			});
		});
		return () => yearRequest.abort();
	});
	$effect.pre(() => {
		const source = data.events,
			nextKey = key;
		untrack(() => {
			const changed = owner !== nextKey;
			if (changed) {
				owner = nextKey;
				events = [];
				loading = true;
				error = '';
				dayDialogOpen = false;
				if (!selectedDate.startsWith(selectedMonth.slice(0, 7))) selectedDate = selectedMonth;
			}
			if (nextKey === `${data.month}|${data.yearId}` && source !== consumed) {
				consumed = source;
				const ticket = request.begin();
				loading = true;
				void source.then((result) => applyEvents(result, ticket.revision));
			} else if (changed) void loadCalendar();
		});
		return () => request.abort();
	});
	async function loadYears() {
		const ticket = yearRequest.begin();
		yearsLoading = true;
		yearError = '';
		const result = await captureRouteLoad(
			listPublicAcademicContextOptions(ticket.signal),
			'โหลดปีการศึกษาไม่สำเร็จ'
		);
		if (!yearRequest.isCurrent(ticket.revision)) return;
		yearsLoading = false;
		if (result.ok) years = result.data.years;
		else yearError = result.error;
	}
	async function loadCalendar() {
		const ticket = request.begin(),
			month = selectedMonth,
			id = selectedYearId;
		loading = true;
		error = '';
		if (id && !years.some((year) => year.id === id)) {
			if (yearsLoading) {
				const result = await data.years;
				if (result.ok) years = result.data;
			} else await loadYears();
		}
		if (!request.isCurrent(ticket.revision)) return;
		const year = years.find((year) => year.id === id);
		if (id && !year) {
			loading = false;
			error = 'ไม่พบปีการศึกษาที่เลือก กรุณาเลือกปีใหม่';
			return;
		}
		const range = publicCalendarRange(month, year);
		const result =
			range.from > range.to
				? { ok: true as const, data: [], error: null }
				: await captureRouteLoad(
						listPublicCalendarEvents(range, { signal: ticket.signal }),
						'โหลดปฏิทินไม่สำเร็จ'
					);
		applyEvents(result, ticket.revision);
	}
	function navigate(month: string, yearId = selectedYearId) {
		const url = new URL(currentUrl);
		url.searchParams.set('month', month.slice(0, 7));
		if (yearId) url.searchParams.set('academicYearId', yearId);
		else url.searchParams.delete('academicYearId');
		void goto(resolve(embedded ? 'calendar/embed' : 'calendar') + url.search, {
			shallow: true,
			state: { ...page.state, publicCalendarUrl: url.href }
		});
	}
	function changeMonth(offset: number) {
		if ((offset < 0 && !canPrevious) || (offset > 0 && !canNext)) return;
		jumpDate = '';
		navigate(toIsoDate(addMonths(new Date(`${monthRange(selectedMonth).from}T00:00:00`), offset)));
	}
	function changeYear(id: string) {
		const year = years.find((year) => year.id === id);
		let month = selectedMonth;
		if (
			year &&
			(month.slice(0, 7) < year.startDate.slice(0, 7) ||
				month.slice(0, 7) > year.endDate.slice(0, 7))
		)
			month = monthRange(year.startDate).from;
		selectedDate =
			year && month.slice(0, 7) === year.startDate.slice(0, 7) ? year.startDate : month;
		jumpDate = '';
		navigate(month, id);
	}
	function goToToday() {
		const today = new Intl.DateTimeFormat('en-CA', {
			timeZone: 'Asia/Bangkok',
			year: 'numeric',
			month: '2-digit',
			day: '2-digit'
		}).format(new Date());
		selectedDate = today;
		jumpDate = '';
		navigate(
			today,
			selectedYear && (today < selectedYear.startDate || today > selectedYear.endDate)
				? ''
				: selectedYearId
		);
	}
	function selectDate(date: string) {
		selectedDate = date;
		if (window.matchMedia('(max-width: 1023px)').matches) dayDialogOpen = true;
	}
	function selectSearchEvent(event: { startDate: string }, searchYearId: string) {
		const year = years.find((year) => year.id === searchYearId);
		selectedDate = year && event.startDate < year.startDate ? year.startDate : event.startDate;
		if (searchYearId === selectedYearId && selectedDate.startsWith(selectedMonth.slice(0, 7)))
			selectDate(selectedDate);
		else {
			jumpDate = selectedDate;
			navigate(selectedDate, searchYearId);
		}
	}
	onDestroy(() => {
		request.abort();
		yearRequest.abort();
	});
</script>

<main
	class={embedded ? 'h-dvh overflow-hidden bg-background' : 'h-dvh overflow-hidden bg-muted/20'}
>
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
				<div class="space-y-1">
					<Label for="public-calendar-year" class="sr-only">ปีการศึกษา</Label>
					<Select.Root type="single" value={selectedYearId} onValueChange={changeYear}>
						<Select.Trigger id="public-calendar-year" class="w-44" disabled={yearsLoading}
							>{selectedYear?.name ?? 'ทุกปีการศึกษา'}</Select.Trigger
						>
						<Select.Content
							><Select.Item value="">ทุกปีการศึกษา</Select.Item
							>{#each years as year (year.id)}<Select.Item value={year.id}>{year.name}</Select.Item
								>{/each}</Select.Content
						>
					</Select.Root>
				</div>
				<Button
					variant="outline"
					size="sm"
					aria-label="ค้นหากิจกรรม"
					disabled={yearsLoading}
					onclick={() => (searchOpen = true)}><Search class="size-4" />ค้นหา</Button
				>
				<Button variant="outline" size="sm" onclick={goToToday}>วันนี้</Button>
				<div class="flex items-center gap-1 sm:gap-2">
					<Button
						variant="outline"
						size="icon-sm"
						onclick={() => changeMonth(-1)}
						disabled={!canPrevious}
						aria-label="เดือนก่อนหน้า"
					>
						<ChevronLeft class="h-4 w-4" />
					</Button>
					<div class="min-w-32 text-center text-sm font-semibold sm:min-w-40">{monthLabel}</div>
					<Button
						variant="outline"
						size="icon-sm"
						onclick={() => changeMonth(1)}
						disabled={!canNext}
						aria-label="เดือนถัดไป"
					>
						<ChevronRight class="h-4 w-4" />
					</Button>
				</div>
			</div>
		</header>

		{#if selectedYear}<p class="shrink-0 text-xs text-muted-foreground">
				{selectedYear.name} · {formatCalendarDate(selectedYear.startDate)} – {formatCalendarDate(
					selectedYear.endDate
				)}
			</p>{/if}
		{#if yearError}<div
				role="alert"
				class="flex shrink-0 flex-wrap items-center gap-2 text-sm text-destructive"
			>
				<span>{yearError}</span><Button size="sm" variant="outline" onclick={loadYears}
					>ลองโหลดปีอีกครั้ง</Button
				>
			</div>{/if}
		{#if loading && !rendered}
			<div class="min-h-0 flex-1 overflow-hidden">
				<PageSkeleton variant="detail" />
			</div>
		{:else if error && !rendered}
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
				aria-busy={loading}
				class="relative grid min-h-0 flex-1 lg:grid-cols-[minmax(0,1fr)_22rem] lg:gap-5 xl:grid-cols-[minmax(0,1fr)_24rem]"
			>
				{#if loading}<RegionUpdatingState label="กำลังโหลดกิจกรรม" />{/if}
				{#if error}<div
						role="alert"
						class="absolute inset-x-0 top-0 z-10 flex flex-wrap items-center justify-center gap-2 rounded-xl border bg-card p-2 text-sm text-destructive"
					>
						<span>{error}</span><Button size="sm" variant="outline" onclick={loadCalendar}
							>ลองอีกครั้ง</Button
						>
					</div>{/if}
				<div class="flex min-h-0 min-w-0 flex-col gap-3">
					<div class="min-h-0 flex-1">
						<CalendarMonthGrid
							monthDate={selectedMonth}
							{events}
							{selectedDate}
							onselect={selectDate}
							hideMobileTimes
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

<CalendarSearchDialog
	bind:open={searchOpen}
	kind="public"
	yearOptions={yearError ? undefined : years}
	yearId={selectedYearId}
	onselect={selectSearchEvent}
/>
