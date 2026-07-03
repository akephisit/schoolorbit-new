<script lang="ts">
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { PageState } from '$lib/components/app-state';
	import { formatCalendarDate } from '$lib/utils/calendar';
	import { Pencil, Trash2 } from 'lucide-svelte';
	import type { CalendarDisplayEvent } from './CalendarMonthGrid.svelte';

	interface CalendarListEvent extends CalendarDisplayEvent {
		categoryName?: string | null;
		description?: string | null;
		location?: string | null;
		allDay: boolean;
		startTime?: string | null;
		endTime?: string | null;
		isPublic: boolean;
	}

	let {
		events = [],
		canManage = false,
		onedit,
		ondelete
	}: {
		events?: CalendarListEvent[];
		canManage?: boolean;
		onedit?: (event: CalendarListEvent) => void;
		ondelete?: (event: CalendarListEvent) => void;
	} = $props();

	const fallbackColor = '#64748b';

	function timeLabel(event: CalendarListEvent) {
		if (event.allDay || !event.startTime || !event.endTime) return '';
		return `${event.startTime.slice(0, 5)}-${event.endTime.slice(0, 5)}`;
	}
</script>

{#if events.length === 0}
	<PageState title="ยังไม่มีกิจกรรม" description="ไม่มีรายการในช่วงวันที่ที่เลือก" />
{:else}
	<div class="space-y-3">
		{#each events as event (event.id)}
			<article class="rounded-md border bg-background p-4">
				<div class="flex items-start justify-between gap-3">
					<div class="min-w-0 flex-1 space-y-2">
						<div class="flex min-w-0 flex-wrap items-center gap-2">
							<span
								class="size-3 shrink-0 rounded-full"
								style={`background-color: ${event.categoryColor ?? fallbackColor}`}
								aria-hidden="true"
							></span>
							<h3 class="min-w-0 truncate font-medium">{event.title}</h3>
							{#if event.isPublic}
								<Badge variant="outline">สาธารณะ</Badge>
							{/if}
						</div>
						<p class="text-sm text-muted-foreground">
							{formatCalendarDate(event.startDate)}
							{#if event.endDate !== event.startDate}
								- {formatCalendarDate(event.endDate)}
							{/if}
							{#if timeLabel(event)}
								<span class="mx-1">·</span>{timeLabel(event)}
							{/if}
						</p>
						{#if event.location}
							<p class="truncate text-sm text-muted-foreground">{event.location}</p>
						{/if}
					</div>
					{#if canManage}
						<div class="flex shrink-0 gap-1">
							<Button
								variant="ghost"
								size="icon"
								onclick={() => onedit?.(event)}
								aria-label={`แก้ไข ${event.title}`}
							>
								<Pencil class="h-4 w-4" />
							</Button>
							<Button
								variant="ghost"
								size="icon"
								onclick={() => ondelete?.(event)}
								aria-label={`ลบ ${event.title}`}
							>
								<Trash2 class="h-4 w-4 text-destructive" />
							</Button>
						</div>
					{/if}
				</div>
			</article>
		{/each}
	</div>
{/if}
