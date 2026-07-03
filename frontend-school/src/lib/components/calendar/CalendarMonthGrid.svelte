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
	import { buildCalendarMonth, eventOverlapsDate } from '$lib/utils/calendar';

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

	let cells = $derived(buildCalendarMonth(monthDate));

	function eventsForDate(date: string) {
		return events.filter((event) => eventOverlapsDate(event, date)).slice(0, 3);
	}
</script>

<div class="overflow-hidden rounded-md border bg-background">
	<div class="grid h-9 grid-cols-7 border-b text-center text-xs font-medium text-muted-foreground">
		{#each weekDays as day (day)}
			<div class="flex items-center justify-center px-2">{day}</div>
		{/each}
	</div>
	<div class="grid grid-cols-7">
		{#each cells as cell (cell.date)}
			<button
				type="button"
				class={cn(
					'flex h-32 min-w-0 flex-col border-b border-r p-2 text-left transition-colors hover:bg-muted/50 focus-visible:z-10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
					!cell.inCurrentMonth && 'bg-muted/30 text-muted-foreground',
					selectedDate === cell.date && 'bg-primary/5 ring-1 ring-primary'
				)}
				aria-pressed={selectedDate === cell.date}
				onclick={() => onselect?.(cell.date)}
			>
				<span class="h-5 text-xs font-medium">{cell.dayNumber}</span>
				<span class="mt-2 flex min-h-0 flex-1 flex-col gap-1 overflow-hidden">
					{#each eventsForDate(cell.date) as event (event.id)}
						<Badge
							variant="secondary"
							class="block h-5 w-full justify-start truncate rounded-md border-l-4 px-1.5 text-[11px] leading-4"
							style={`border-left-color: ${event.categoryColor ?? fallbackColor}`}
						>
							{event.title}
						</Badge>
					{/each}
				</span>
			</button>
		{/each}
	</div>
</div>
