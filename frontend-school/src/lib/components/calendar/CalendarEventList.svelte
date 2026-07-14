<script lang="ts">
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { PageState } from '$lib/components/app-state';
	import { cn } from '$lib/utils';
	import { formatCalendarDate } from '$lib/utils/calendar';
	import { CalendarRange, Clock3, Globe2, MapPin, Pencil, Trash2, Users } from 'lucide-svelte';
	import type { CalendarDisplayEvent } from './CalendarMonthGrid.svelte';

	interface CalendarListEvent extends CalendarDisplayEvent {
		categoryName?: string | null;
		description?: string | null;
		location?: string | null;
		allDay: boolean;
		startTime?: string | null;
		endTime?: string | null;
		isPublic: boolean;
		tags?: { id: string; name: string }[];
		targets?: { audienceType: string }[];
	}

	let {
		events = [],
		canManage = false,
		showFullDescription = false,
		onedit,
		ondelete
	}: {
		events?: CalendarListEvent[];
		canManage?: boolean;
		showFullDescription?: boolean;
		onedit?: (event: CalendarListEvent) => void;
		ondelete?: (event: CalendarListEvent) => void;
	} = $props();

	const fallbackColor = '#64748b';
	const audienceLabels: Record<string, string> = {
		all: 'ทุกคน',
		staff: 'บุคลากร',
		student: 'นักเรียน',
		parent: 'ผู้ปกครอง'
	};

	function timeLabel(event: CalendarListEvent) {
		if (event.allDay || !event.startTime || !event.endTime) return '';
		return `${event.startTime.slice(0, 5)}-${event.endTime.slice(0, 5)}`;
	}

	function audienceLabel(event: CalendarListEvent) {
		const labels = [
			...new Set(
				(event.targets ?? []).map(
					(target) => audienceLabels[target.audienceType] ?? target.audienceType
				)
			)
		];
		return labels.join(', ');
	}
</script>

{#if events.length === 0}
	<PageState title="ยังไม่มีกิจกรรม" description="ไม่มีรายการในช่วงวันที่ที่เลือก" />
{:else}
	<div class="space-y-3">
		{#each events as event (event.id)}
			<article
				class="rounded-xl border border-l-4 bg-card p-4 shadow-sm transition-shadow hover:shadow-md"
				style:border-left-color={event.categoryColor ?? fallbackColor}
			>
				<div class="flex items-start justify-between gap-3">
					<div class="min-w-0 flex-1 space-y-3">
						<div class="flex min-w-0 flex-wrap items-center gap-2">
							<h3 class="min-w-0 flex-1 text-base font-semibold leading-snug">{event.title}</h3>
							{#if event.categoryName}
								<Badge variant="secondary" class="font-normal">{event.categoryName}</Badge>
							{/if}
							{#if event.isPublic}
								<Badge variant="outline" class="gap-1 font-normal">
									<Globe2 class="size-3" />
									สาธารณะ
								</Badge>
							{/if}
						</div>
						{#if (event.tags?.length ?? 0) > 0}
							<div class="flex flex-wrap gap-1.5">
								{#each event.tags ?? [] as tag (tag.id)}
									<Badge variant="outline" class="font-normal">#{tag.name}</Badge>
								{/each}
							</div>
						{/if}

						<div class="grid gap-2 text-sm text-muted-foreground">
							<p class="flex items-start gap-2">
								<CalendarRange class="mt-0.5 size-4 shrink-0" />
								<span>
									{formatCalendarDate(event.startDate)}
									{#if event.endDate !== event.startDate}
										– {formatCalendarDate(event.endDate)}
									{/if}
								</span>
							</p>
							<p class="flex items-center gap-2">
								<Clock3 class="size-4 shrink-0" />
								<span>{event.allDay ? 'ทั้งวัน' : timeLabel(event)}</span>
							</p>
							{#if event.location}
								<p class="flex items-start gap-2">
									<MapPin class="mt-0.5 size-4 shrink-0" />
									<span>{event.location}</span>
								</p>
							{/if}
							{#if audienceLabel(event)}
								<p class="flex items-start gap-2">
									<Users class="mt-0.5 size-4 shrink-0" />
									<span>{audienceLabel(event)}</span>
								</p>
							{/if}
						</div>

						{#if event.description}
							<p
								class={cn(
									'whitespace-pre-line text-sm leading-relaxed text-foreground/80',
									!showFullDescription && 'line-clamp-3'
								)}
							>
								{event.description}
							</p>
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
