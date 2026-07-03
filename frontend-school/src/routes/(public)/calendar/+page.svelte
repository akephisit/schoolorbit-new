<script lang="ts">
	import { onMount } from 'svelte';
	import { addMonths } from 'date-fns';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import CalendarMonthGrid from '$lib/components/calendar/CalendarMonthGrid.svelte';
	import CalendarEventList from '$lib/components/calendar/CalendarEventList.svelte';
	import { type CalendarPublicEvent, listPublicCalendarEvents } from '$lib/api/calendar';
	import {
		eventOverlapsDate,
		formatCalendarDate,
		monthRange,
		toIsoDate
	} from '$lib/utils/calendar';
	import { CalendarDays, ChevronLeft, ChevronRight } from 'lucide-svelte';

	let { data } = $props();

	let events = $state<CalendarPublicEvent[]>([]);
	let loading = $state(true);
	let error = $state('');
	let selectedMonth = $state(toIsoDate(new Date()));
	let selectedDate = $state(toIsoDate(new Date()));

	const monthLabel = $derived(formatCalendarDate(monthRange(selectedMonth).from));
	const selectedDateEvents = $derived(
		events
			.filter((event) => eventOverlapsDate(event, selectedDate))
			.sort((left, right) => left.startDate.localeCompare(right.startDate))
	);

	async function loadCalendar() {
		loading = true;
		error = '';
		try {
			events = await listPublicCalendarEvents({ ...monthRange(selectedMonth) });
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

	onMount(loadCalendar);
</script>

<svelte:head>
	<title>{data.title}</title>
</svelte:head>

<main class="min-h-screen bg-muted/20">
	<section class="mx-auto flex w-full max-w-7xl flex-col gap-6 px-4 py-6 lg:px-8">
		<header class="flex flex-col gap-4 border-b pb-5 sm:flex-row sm:items-end sm:justify-between">
			<div class="space-y-2">
				<div class="flex items-center gap-2 text-sm text-muted-foreground">
					<CalendarDays class="h-4 w-4" />
					ปฏิทินสาธารณะ
				</div>
				<h1 class="text-3xl font-semibold tracking-tight">ปฏิทินโรงเรียน</h1>
				<p class="text-sm text-muted-foreground">กิจกรรมที่โรงเรียนเปิดเผยต่อสาธารณะ</p>
			</div>
			<div class="flex items-center gap-2">
				<Button
					variant="outline"
					size="icon"
					onclick={() => changeMonth(-1)}
					aria-label="เดือนก่อนหน้า"
				>
					<ChevronLeft class="h-4 w-4" />
				</Button>
				<div class="min-w-44 text-center text-sm font-medium">{monthLabel}</div>
				<Button
					variant="outline"
					size="icon"
					onclick={() => changeMonth(1)}
					aria-label="เดือนถัดไป"
				>
					<ChevronRight class="h-4 w-4" />
				</Button>
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
			<div class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_380px]">
				<CalendarMonthGrid
					monthDate={selectedMonth}
					{events}
					{selectedDate}
					onselect={(date) => (selectedDate = date)}
				/>
				<section class="space-y-3">
					<div>
						<h2 class="text-lg font-semibold">กิจกรรมวันที่ {formatCalendarDate(selectedDate)}</h2>
						<p class="text-sm text-muted-foreground">
							{selectedDateEvents.length} รายการในวันที่เลือก
						</p>
					</div>
					<CalendarEventList events={selectedDateEvents} canManage={false} />
				</section>
			</div>
		{/if}
	</section>
</main>
