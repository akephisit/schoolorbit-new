<script lang="ts" module>
	export interface CalendarDisplayEvent {
		id: string;
		title: string;
		startDate: string;
		endDate: string;
		allDay?: boolean;
		startTime?: string | null;
		categoryColor?: string | null;
	}
</script>

<script lang="ts">
	import { cn } from '$lib/utils';
	import {
		CALENDAR_WEEKDAY_LABELS,
		buildCalendarMonthWeeks,
		eventOverlapsDate,
		formatCalendarDate,
		toIsoDate
	} from '$lib/utils/calendar';
	import type { CalendarWeekEventSegment } from '$lib/utils/calendar';

	let {
		monthDate,
		events = [],
		selectedDate = '',
		onselect
	}: {
		monthDate: string;
		events?: CalendarDisplayEvent[];
		selectedDate?: string;
		onselect?: (date: string) => void;
	} = $props();

	const fallbackColor = '#64748b';
	const todayDate = toIsoDate(new Date());

	let weeks = $derived(buildCalendarMonthWeeks(monthDate, events));

	function eventsForDate(date: string) {
		return events.filter((event) => eventOverlapsDate(event, date));
	}

	function segmentLabel(segment: CalendarWeekEventSegment<CalendarDisplayEvent>) {
		const startTime = segment.event.startTime?.slice(0, 5);
		if (segment.event.allDay || !startTime || segment.continuesFromPreviousWeek) {
			return segment.event.title;
		}
		return `${startTime} ${segment.event.title}`;
	}
</script>

<div class="overflow-hidden rounded-xl border bg-card shadow-sm">
	<div
		class="grid h-10 grid-cols-7 border-b bg-muted/30 text-center text-[11px] font-medium text-muted-foreground sm:text-xs"
	>
		{#each CALENDAR_WEEKDAY_LABELS as day (day)}
			<div class="flex items-center justify-center px-2">{day}</div>
		{/each}
	</div>
	<div>
		{#each weeks as week, weekIndex (week.cells[0]?.date ?? weekIndex)}
			<div class="relative grid grid-cols-7">
				{#each week.cells as cell, dayIndex (cell.date)}
					{@const dayEvents = eventsForDate(cell.date)}
					<button
						type="button"
						class={cn(
							'group relative flex h-16 min-w-0 flex-col border-b border-r p-1.5 text-left transition-colors hover:bg-muted/50 focus-visible:z-20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring sm:h-28 sm:p-2',
							!cell.inCurrentMonth && 'bg-muted/30 text-muted-foreground',
							selectedDate === cell.date && 'bg-primary/5 ring-2 ring-inset ring-primary'
						)}
						aria-pressed={selectedDate === cell.date}
						aria-label={`${formatCalendarDate(cell.date)}, ${dayEvents.length} กิจกรรม`}
						onclick={() => onselect?.(cell.date)}
					>
						<span
							class={cn(
								'flex size-6 items-center justify-center rounded-full text-xs font-medium transition-colors',
								cell.date === todayDate && 'bg-primary text-primary-foreground',
								selectedDate === cell.date && cell.date !== todayDate && 'text-primary'
							)}
						>
							{cell.dayNumber}
						</span>

						{#if (week.hiddenEventCounts[dayIndex] ?? 0) > 0}
							<span
								class="absolute bottom-0.5 right-1 z-20 rounded bg-card/90 px-1 text-[9px] font-medium text-muted-foreground shadow-sm sm:bottom-1 sm:text-[10px]"
							>
								+{week.hiddenEventCounts[dayIndex]}
							</span>
						{/if}
					</button>
				{/each}

				<div
					class="pointer-events-none absolute inset-x-0 top-9 z-10 grid auto-rows-[6px] grid-cols-7 gap-y-0.5 sm:auto-rows-[18px]"
					aria-hidden="true"
				>
					{#each week.segments as segment (`${segment.event.id}-${weekIndex}`)}
						<div
							class={cn(
								'min-w-0 overflow-hidden text-white shadow-sm',
								segment.continuesFromPreviousWeek ? 'ml-0 rounded-l-none' : 'ml-0.5 rounded-l-sm',
								segment.continuesIntoNextWeek ? 'mr-0 rounded-r-none' : 'mr-0.5 rounded-r-sm'
							)}
							style:grid-column={`${segment.startColumn + 1} / span ${segment.span}`}
							style:grid-row={`${segment.lane + 1}`}
							style:background-color={segment.event.categoryColor ?? fallbackColor}
							title={segmentLabel(segment)}
						>
							<span
								class="hidden truncate px-1.5 text-[10px] font-medium leading-[18px] sm:block xl:text-[11px]"
							>
								{segmentLabel(segment)}
							</span>
						</div>
					{/each}
				</div>
			</div>
		{/each}
	</div>
</div>
