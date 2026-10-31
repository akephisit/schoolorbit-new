<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { addMonths } from 'date-fns';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import CalendarSearchDialog from '#lib/components/calendar/CalendarSearchDialog.svelte';
	import CalendarColorKey from '#lib/components/calendar/CalendarColorKey.svelte';
	import CalendarDayTimelineDialog from '#lib/components/calendar/CalendarDayTimelineDialog.svelte';
	import CalendarMonthGrid from '#lib/components/calendar/CalendarMonthGrid.svelte';
	import CalendarEventList from '#lib/components/calendar/CalendarEventList.svelte';
	import { type CalendarPublicEvent, listPublicCalendarEvents } from '#lib/api/calendar.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import type { PublicCalendarRouteData } from '#lib/calendar/public-route.js';
	import { calendarRouteFilters } from '#lib/utils/calendar-route-filters.js';
	import {
		publicCalendarRange,
		buildCalendarColorKey,
		eventOverlapsDate,
		formatCalendarMonth,
		formatCalendarDate,
		monthRange,
		toIsoDate
	} from '#lib/utils/calendar.js';
	import { ArrowLeft, CalendarDays, ChevronLeft, ChevronRight, Search, X } from '@lucide/svelte';

	type PublicCalendarMode = 'page' | 'embed';
	let { mode = 'page', data }: { mode?: PublicCalendarMode; data: PublicCalendarRouteData } =
		$props();
	const embedded = $derived(mode === 'embed');
	const currentUrl = $derived(
		page.state.publicCalendarUrl ? new URL(page.state.publicCalendarUrl) : new URL(page.url.href)
	);
	const selectedMonth = $derived(calendarRouteFilters(currentUrl).month);
	const key = $derived(selectedMonth);
	let events = $state.raw<CalendarPublicEvent[]>([]);
	let loading = $state(true),
		rendered = $state(false),
		error = $state('');
	let selectedDate = $state(''),
		dayDialogOpen = $state(false),
		searchOpen = $state(false);
	let selectedAnchor: HTMLElement | null = null;
	let detailsElement = $state.raw<HTMLElement | null>(null);
	let owner = '',
		jumpDate = '',
		consumed: typeof data.events | null = null;
	const request = new LatestRequest();
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

	function applyEvents(result: Awaited<typeof data.events>, revision: number) {
		if (!request.isCurrent(revision)) return;
		loading = false;
		if (result.ok) {
			events = result.data;
			rendered = true;
			error = '';
			if (jumpDate && jumpDate.startsWith(selectedMonth.slice(0, 7))) {
				selectDate(jumpDate);
				jumpDate = '';
			}
		} else error = result.error;
	}
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
				selectedDate = '';
				if (!jumpDate.startsWith(selectedMonth.slice(0, 7))) jumpDate = '';
			}
			if (nextKey === data.month && source !== consumed) {
				consumed = source;
				const ticket = request.begin();
				loading = true;
				void source.then((result) => applyEvents(result, ticket.revision));
			} else if (changed) void loadCalendar();
		});
		return () => request.abort();
	});
	async function loadCalendar() {
		const ticket = request.begin();
		loading = true;
		error = '';
		const result = await captureRouteLoad(
			listPublicCalendarEvents(publicCalendarRange(selectedMonth), { signal: ticket.signal }),
			'โหลดปฏิทินไม่สำเร็จ'
		);
		applyEvents(result, ticket.revision);
	}
	function navigate(month: string) {
		const url = new URL(currentUrl);
		url.searchParams.set('month', month.slice(0, 7));
		url.searchParams.delete('academicYearId');
		void goto(resolve(embedded ? 'calendar/embed' : 'calendar') + url.search, {
			shallow: true,
			state: { ...page.state, publicCalendarUrl: url.href }
		});
	}
	function changeMonth(offset: number) {
		clearSelection();
		navigate(toIsoDate(addMonths(new Date(`${monthRange(selectedMonth).from}T00:00:00`), offset)));
	}

	function goToToday() {
		const today = new Intl.DateTimeFormat('en-CA', {
			timeZone: 'Asia/Bangkok',
			year: 'numeric',
			month: '2-digit',
			day: '2-digit'
		}).format(new Date());
		clearSelection();
		navigate(today);
	}
	function selectDate(date: string, anchor?: HTMLElement) {
		selectedDate = date;
		selectedAnchor =
			anchor ?? document.querySelector<HTMLButtonElement>(`[data-calendar-date="${date}"]`);
		if (window.matchMedia('(max-width: 1023px)').matches) dayDialogOpen = true;
	}
	function closeDetails() {
		const restoreFocus = detailsElement?.contains(document.activeElement);
		const anchor = selectedAnchor;
		clearSelection();
		if (restoreFocus && anchor?.isConnected) anchor.focus();
	}
	function clearSelection() {
		selectedAnchor = null;
		selectedDate = '';
		jumpDate = '';
		dayDialogOpen = false;
	}
	function dismissOutside(event: PointerEvent) {
		if (dayDialogOpen || searchOpen || !(event.target instanceof Element)) return;
		if (event.target.closest('[data-calendar-date], [data-public-calendar-details]')) return;
		clearSelection();
	}
	function dismissWithEscape(event: KeyboardEvent) {
		if (event.key === 'Escape' && !dayDialogOpen && !searchOpen) closeDetails();
	}
	function setDayDialogOpen(open: boolean) {
		dayDialogOpen = open;
		if (!open) clearSelection();
	}
	function selectSearchEvent(event: { startDate: string }) {
		const date = event.startDate;
		if (date.startsWith(selectedMonth.slice(0, 7))) selectDate(date);
		else {
			selectedDate = '';
			jumpDate = date;
			navigate(date);
		}
	}

	onDestroy(() => {
		request.abort();
	});
</script>

<svelte:window onpointerdown={dismissOutside} onkeydown={dismissWithEscape} />

<main
	class={embedded ? 'h-dvh overflow-y-auto bg-background' : 'h-dvh overflow-y-auto bg-muted/20'}
>
	<section
		data-calendar-mode={mode}
		class={embedded
			? 'flex h-full min-h-fit w-full flex-col gap-2 p-2 sm:gap-3 sm:p-3'
			: 'mx-auto flex h-full min-h-fit w-full max-w-screen-2xl flex-col gap-3 px-3 py-3 sm:px-4 lg:gap-4 lg:px-8 lg:py-4 2xl:px-10'}
	>
		<header
			class={embedded
				? 'flex shrink-0 items-center justify-between gap-2 border-b pb-2'
				: 'flex shrink-0 flex-col gap-2 border-b pb-3 sm:flex-row sm:items-end sm:justify-between'}
		>
			{#if !embedded}
				<div class="flex flex-wrap items-center gap-3">
					<Button href={resolve('/')} variant="outline" size="sm"
						><ArrowLeft class="size-4" />กลับหน้าแรก</Button
					>
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
				<Button
					variant="outline"
					size="sm"
					aria-label="ค้นหากิจกรรม"
					onclick={() => (searchOpen = true)}><Search class="size-4" />ค้นหา</Button
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
				class="relative grid min-h-[28rem] flex-1 sm:min-h-[30rem] lg:grid-cols-[minmax(0,1fr)_22rem] lg:gap-5 xl:grid-cols-[minmax(0,1fr)_24rem]"
			>
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
					bind:this={detailsElement}
					data-public-calendar-details
					class="hidden min-h-0 flex-col overflow-hidden rounded-xl border bg-card shadow-sm lg:flex"
				>
					{#if selectedDate}
						<div class="flex shrink-0 items-end justify-between gap-3 border-b px-3 py-2.5 sm:px-4">
							<div>
								<p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
									วันที่เลือก
								</p>
								<h2 class="mt-1 text-lg font-semibold">{formatCalendarDate(selectedDate)}</h2>
							</div>
							<div class="flex shrink-0 items-center gap-2">
								<span class="text-sm text-muted-foreground">{selectedDateEvents.length} รายการ</span
								>
								<Button
									variant="ghost"
									size="icon-sm"
									aria-label="ปิดรายละเอียดกิจกรรม"
									onclick={closeDetails}><X class="size-4" /></Button
								>
							</div>
						</div>
						<div class="min-h-0 flex-1 overflow-y-auto p-3 sm:p-4">
							<CalendarEventList
								events={selectedDateEvents}
								canManage={false}
								showFullDescription
							/>
						</div>
					{:else}
						<div
							class="flex flex-1 items-center justify-center p-4 text-center text-sm text-muted-foreground"
						>
							เลือกวันที่เพื่อดูกิจกรรม
						</div>
					{/if}
				</aside>
			</div>
		{/if}
	</section>
</main>

<CalendarDayTimelineDialog
	bind:open={() => dayDialogOpen, setDayDialogOpen}
	date={selectedDate || selectedMonth}
	events={selectedDateEvents}
/>

<CalendarSearchDialog bind:open={searchOpen} kind="public" onselect={selectSearchEvent} />
