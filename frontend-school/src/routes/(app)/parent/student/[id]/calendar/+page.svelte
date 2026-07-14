<script lang="ts">
	import type { PageProps } from './$types';
	import { onMount } from 'svelte';
	import { addMonths } from 'date-fns';
	import { toast } from 'svelte-sonner';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import CalendarMonthGrid from '$lib/components/calendar/CalendarMonthGrid.svelte';
	import CalendarEventList from '$lib/components/calendar/CalendarEventList.svelte';
	import { type CalendarViewerEvent, listChildCalendarEvents } from '$lib/api/calendar';
	import {
		calendarGridRange,
		eventOverlapsDate,
		formatCalendarDate,
		formatCalendarMonth,
		monthRange,
		toIsoDate
	} from '$lib/utils/calendar';
	import { ChevronLeft, ChevronRight } from 'lucide-svelte';

	let { data }: PageProps = $props();

	let events = $state.raw<CalendarViewerEvent[]>([]);
	let loading = $state(true);
	let error = $state('');
	let selectedMonth = $state(toIsoDate(new Date()));
	let selectedDate = $state(toIsoDate(new Date()));

	const monthLabel = $derived(formatCalendarMonth(selectedMonth));
	const selectedDateEvents = $derived(
		events
			.filter((event) => eventOverlapsDate(event, selectedDate))
			.sort((left, right) => left.startDate.localeCompare(right.startDate))
	);

	async function loadCalendar() {
		loading = true;
		error = '';
		try {
			events = await listChildCalendarEvents(data.studentId, {
				...calendarGridRange(selectedMonth)
			});
		} catch (loadError: unknown) {
			error =
				(loadError instanceof Error ? loadError.message : String(loadError)) ||
				'โหลดปฏิทินไม่สำเร็จ';
			toast.error(error);
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

	onMount(() => {
		void loadCalendar();
	});
</script>

<svelte:head>
	<title>{data.title}</title>
</svelte:head>

<PageShell
	title="ปฏิทินของลูก"
	description="กิจกรรมที่เกี่ยวข้องกับนักเรียน"
	backHref={`/parent/student/${data.studentId}`}
>
	<div
		class="flex flex-wrap items-center justify-between gap-3 rounded-md border bg-background p-4"
	>
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
			<Button variant="outline" size="icon" onclick={() => changeMonth(1)} aria-label="เดือนถัดไป">
				<ChevronRight class="h-4 w-4" />
			</Button>
		</div>
		<Button variant="ghost" onclick={loadCalendar}>รีเฟรช</Button>
	</div>

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
</PageShell>
