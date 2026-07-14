<script lang="ts">
	import { onMount } from 'svelte';
	import { addMonths } from 'date-fns';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import CalendarMonthGrid from '$lib/components/calendar/CalendarMonthGrid.svelte';
	import CalendarEventList from '$lib/components/calendar/CalendarEventList.svelte';
	import { type CalendarPublicEvent, listPublicCalendarEvents } from '$lib/api/calendar';
	import {
		calendarGridRange,
		eventOverlapsDate,
		formatCalendarDate,
		formatCalendarMonth,
		monthRange,
		toIsoDate
	} from '$lib/utils/calendar';
	import { CalendarDays, ChevronLeft, ChevronRight } from 'lucide-svelte';

	let { data } = $props();

	let events = $state.raw<CalendarPublicEvent[]>([]);
	let loading = $state(true);
	let error = $state('');
	let selectedMonth = $state(toIsoDate(new Date()));
	let selectedDate = $state(toIsoDate(new Date()));

	const monthLabel = $derived(formatCalendarMonth(selectedMonth));
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

	onMount(() => {
		void loadCalendar();
	});
</script>

<svelte:head>
	<title>{data.title}</title>
</svelte:head>

<main class="flex min-h-svh flex-col bg-background lg:h-svh lg:overflow-hidden">
	<header
		class="sticky top-0 z-40 shrink-0 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/85 lg:static lg:flex lg:min-h-16 lg:items-center lg:justify-between"
	>
		<div class="flex min-h-14 items-center gap-3 px-3 sm:px-4 lg:px-5">
			<div
				class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary"
			>
				<CalendarDays class="size-5" />
			</div>
			<div class="min-w-0">
				<h1 class="truncate text-lg font-semibold tracking-tight sm:text-xl">ปฏิทินโรงเรียน</h1>
				<p class="hidden text-xs text-muted-foreground sm:block">
					กิจกรรมที่โรงเรียนเปิดเผยต่อสาธารณะ
				</p>
			</div>
		</div>

		<div
			class="flex min-h-12 items-center justify-between gap-2 border-t px-3 sm:px-4 lg:justify-end lg:border-t-0 lg:px-5"
		>
			<Button variant="outline" size="sm" onclick={goToToday}>วันนี้</Button>
			<div class="flex items-center gap-1 sm:gap-2">
				<Button
					variant="ghost"
					size="icon-sm"
					onclick={() => changeMonth(-1)}
					aria-label="เดือนก่อนหน้า"
				>
					<ChevronLeft class="h-4 w-4" />
				</Button>
				<div class="min-w-32 text-center text-sm font-semibold sm:min-w-40">{monthLabel}</div>
				<Button
					variant="ghost"
					size="icon-sm"
					onclick={() => changeMonth(1)}
					aria-label="เดือนถัดไป"
				>
					<ChevronRight class="h-4 w-4" />
				</Button>
			</div>
		</div>
	</header>

	<div class="min-h-0 flex-1">
		{#if loading}
			<div class="h-full overflow-auto p-4 lg:p-6">
				<PageSkeleton variant="detail" />
			</div>
		{:else if error}
			<div class="p-4 lg:p-6">
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
				class="hidden h-full min-h-0 grid-cols-[minmax(0,1fr)_22rem] lg:grid xl:grid-cols-[minmax(0,1fr)_25rem]"
			>
				<CalendarMonthGrid
					monthDate={selectedMonth}
					{events}
					{selectedDate}
					onselect={(date) => (selectedDate = date)}
					fillHeight
					class="rounded-none border-0 shadow-none"
				/>
				<aside class="min-h-0 overflow-y-auto border-l bg-muted/15">
					<div class="sticky top-0 z-20 border-b bg-background/95 px-5 py-4 backdrop-blur">
						<p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
							วันที่เลือก
						</p>
						<div class="mt-1 flex items-baseline justify-between gap-3">
							<h2 class="text-lg font-semibold">{formatCalendarDate(selectedDate)}</h2>
							<span class="shrink-0 text-sm text-muted-foreground">
								{selectedDateEvents.length} รายการ
							</span>
						</div>
					</div>
					<div class="p-4">
						<CalendarEventList events={selectedDateEvents} canManage={false} showFullDescription />
					</div>
				</aside>
			</div>

			<div class="lg:hidden">
				<CalendarMonthGrid
					monthDate={selectedMonth}
					{events}
					{selectedDate}
					onselect={(date) => (selectedDate = date)}
					class="rounded-none border-x-0 border-t-0 shadow-none"
				/>
				<section class="border-t bg-muted/15">
					<div class="sticky top-26 z-30 border-b bg-background/95 px-4 py-3 backdrop-blur">
						<div class="flex items-baseline justify-between gap-3">
							<h2 class="text-base font-semibold">{formatCalendarDate(selectedDate)}</h2>
							<span class="shrink-0 text-xs text-muted-foreground">
								{selectedDateEvents.length} รายการ
							</span>
						</div>
					</div>
					<div class="p-3 sm:p-4">
						<CalendarEventList events={selectedDateEvents} canManage={false} showFullDescription />
					</div>
				</section>
			</div>
		{/if}
	</div>
</main>
