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

<main class="min-h-screen bg-muted/20">
	<section
		class="mx-auto flex w-full max-w-7xl flex-col gap-5 px-3 py-4 sm:px-4 sm:py-5 lg:gap-6 lg:px-6 lg:py-6"
	>
		<header class="flex flex-col gap-4 border-b pb-5 sm:flex-row sm:items-end sm:justify-between">
			<div class="flex items-center gap-3">
				<div
					class="flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary"
				>
					<CalendarDays class="size-5" />
				</div>
				<div class="min-w-0">
					<h1 class="text-2xl font-semibold tracking-tight">ปฏิทินโรงเรียน</h1>
					<p class="text-sm text-muted-foreground">กิจกรรมที่โรงเรียนเปิดเผยต่อสาธารณะ</p>
				</div>
			</div>

			<div class="flex flex-wrap items-center justify-between gap-2 sm:justify-end">
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
			<PageSkeleton variant="detail" />
		{:else if error}
			<PageState
				variant="error"
				title="โหลดปฏิทินไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={loadCalendar}
			/>
		{:else}
			<div class="grid items-start gap-5 xl:grid-cols-[minmax(0,1fr)_24rem]">
				<CalendarMonthGrid
					monthDate={selectedMonth}
					{events}
					{selectedDate}
					onselect={(date) => (selectedDate = date)}
				/>
				<aside class="space-y-3">
					<div class="flex items-end justify-between gap-3">
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
					<CalendarEventList events={selectedDateEvents} canManage={false} showFullDescription />
				</aside>
			</div>
		{/if}
	</section>
</main>
