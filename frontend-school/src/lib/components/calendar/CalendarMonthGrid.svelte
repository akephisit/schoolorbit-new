<script lang="ts" module>
	export interface CalendarDisplayEvent {
		id: string;
		title: string;
		startDate: string;
		endDate: string;
		categoryColor?: string | null;
	}
</script>

<script lang="ts">
	import { Badge } from '$lib/components/ui/badge';
	import { cn } from '$lib/utils';
	import {
		buildCalendarMonth,
		eventOverlapsDate,
		formatCalendarDate,
		toIsoDate
	} from '$lib/utils/calendar';

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

	const weekDays = ['จ', 'อ', 'พ', 'พฤ', 'ศ', 'ส', 'อา'];
	const fallbackColor = '#64748b';
	const todayDate = toIsoDate(new Date());

	let cells = $derived(buildCalendarMonth(monthDate));

	function eventsForDate(date: string) {
		return events.filter((event) => eventOverlapsDate(event, date));
	}
</script>

<div class="overflow-hidden rounded-xl border bg-card shadow-sm">
	<div
		class="grid h-10 grid-cols-7 border-b bg-muted/30 text-center text-[11px] font-medium text-muted-foreground sm:text-xs"
	>
		{#each weekDays as day (day)}
			<div class="flex items-center justify-center px-2">{day}</div>
		{/each}
	</div>
	<div class="grid grid-cols-7">
		{#each cells as cell (cell.date)}
			{@const dayEvents = eventsForDate(cell.date)}
			{@const visibleEvents = dayEvents.slice(0, 3)}
			<button
				type="button"
				class={cn(
					'group relative flex h-16 min-w-0 flex-col border-b border-r p-1.5 text-left transition-colors hover:bg-muted/50 focus-visible:z-10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring sm:h-24 sm:p-2 xl:h-28',
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

				<span class="mt-auto flex w-full items-center gap-1 pb-0.5 sm:hidden">
					{#each visibleEvents as event (event.id)}
						<span
							class="size-1.5 rounded-full"
							style:background-color={event.categoryColor ?? fallbackColor}
							aria-hidden="true"
						></span>
					{/each}
					{#if dayEvents.length > 3}
						<span class="text-[9px] text-muted-foreground">+{dayEvents.length - 3}</span>
					{/if}
				</span>

				<span class="mt-1 hidden min-h-0 flex-1 flex-col gap-1 overflow-hidden sm:flex">
					{#each visibleEvents as event (event.id)}
						<Badge
							variant="secondary"
							class="block h-5 w-full justify-start truncate rounded-md border-l-[3px] bg-muted/70 px-1.5 text-[10px] font-normal leading-4 xl:text-[11px]"
							style={`border-left-color: ${event.categoryColor ?? fallbackColor}`}
						>
							{event.title}
						</Badge>
					{/each}
					{#if dayEvents.length > 3}
						<span class="px-1 text-[10px] text-muted-foreground">
							อีก {dayEvents.length - 3} รายการ
						</span>
					{/if}
				</span>
			</button>
		{/each}
	</div>
</div>
