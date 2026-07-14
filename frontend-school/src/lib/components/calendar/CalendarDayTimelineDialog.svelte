<script lang="ts">
	import * as Dialog from '$lib/components/ui/dialog';
	import { Badge } from '$lib/components/ui/badge';
	import { cn } from '$lib/utils';
	import { formatCalendarDate } from '$lib/utils/calendar';
	import { CalendarRange, Clock3, MapPin } from 'lucide-svelte';
	import type { CalendarDisplayEvent } from './CalendarMonthGrid.svelte';

	interface CalendarTimelineEvent extends CalendarDisplayEvent {
		categoryName?: string | null;
		description?: string | null;
		location?: string | null;
		allDay: boolean;
		endTime?: string | null;
		tags?: { id: string; name: string }[];
	}

	let {
		open = $bindable(false),
		date,
		events = []
	}: {
		open: boolean;
		date: string;
		events?: CalendarTimelineEvent[];
	} = $props();

	const fallbackColor = '#64748b';
	const allDayEvents = $derived(events.filter((event) => event.allDay));
	const timedEvents = $derived(
		events
			.filter((event) => !event.allDay)
			.sort(
				(left, right) =>
					(left.startTime ?? '').localeCompare(right.startTime ?? '') ||
					left.title.localeCompare(right.title, 'th')
			)
	);

	function shortTime(value: string | null | undefined) {
		return value?.slice(0, 5) ?? '';
	}

	function timeRange(event: CalendarTimelineEvent) {
		const startTime = shortTime(event.startTime);
		const endTime = shortTime(event.endTime);
		if (!startTime && !endTime) return 'ไม่ระบุเวลา';
		if (!endTime) return startTime;
		return `${startTime} – ${endTime}`;
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content
		class="flex h-[min(90dvh,46rem)] max-h-[calc(100dvh-1rem)] max-w-[calc(100%-1rem)] flex-col gap-0 overflow-hidden rounded-2xl p-0 sm:max-w-lg"
	>
		<Dialog.Header class="shrink-0 border-b px-5 py-4 pr-12 text-left">
			<Dialog.Title class="text-xl">{formatCalendarDate(date)}</Dialog.Title>
			<Dialog.Description>
				{events.length === 0 ? 'ไม่มีกิจกรรมในวันนี้' : `${events.length} กิจกรรมในวันนี้`}
			</Dialog.Description>
		</Dialog.Header>

		<div class="min-h-0 flex-1 overflow-y-auto overscroll-contain px-4 py-4">
			{#if events.length === 0}
				<div class="flex min-h-56 flex-col items-center justify-center text-center">
					<div
						class="flex size-12 items-center justify-center rounded-full bg-muted text-muted-foreground"
					>
						<CalendarRange class="size-5" />
					</div>
					<p class="mt-3 font-medium">วันนี้ยังไม่มีกิจกรรม</p>
					<p class="mt-1 text-sm text-muted-foreground">เลือกวันอื่นจากปฏิทินเพื่อดูรายการ</p>
				</div>
			{:else}
				<div class="space-y-5">
					{#if allDayEvents.length > 0}
						<section aria-labelledby="all-day-events-heading">
							<div class="mb-2 flex items-center justify-between gap-3">
								<h3
									id="all-day-events-heading"
									class="text-xs font-semibold uppercase tracking-wide text-muted-foreground"
								>
									ทั้งวัน
								</h3>
								<span class="text-xs text-muted-foreground">{allDayEvents.length} รายการ</span>
							</div>
							<div class="space-y-2">
								{#each allDayEvents as event (event.id)}
									<article
										class="rounded-lg border border-l-4 bg-card px-3 py-2.5 shadow-sm"
										style:border-left-color={event.categoryColor ?? fallbackColor}
									>
										<div class="flex min-w-0 items-start justify-between gap-2">
											<h4 class="min-w-0 flex-1 font-semibold leading-snug">{event.title}</h4>
											{#if event.categoryName}
												<Badge variant="secondary" class="shrink-0 font-normal">
													{event.categoryName}
												</Badge>
											{/if}
										</div>
										{#if event.location}
											<p class="mt-1.5 flex items-start gap-1.5 text-sm text-muted-foreground">
												<MapPin class="mt-0.5 size-3.5 shrink-0" />
												<span>{event.location}</span>
											</p>
										{/if}
										{#if event.startDate !== event.endDate}
											<p class="mt-1 flex items-start gap-1.5 text-sm text-muted-foreground">
												<CalendarRange class="mt-0.5 size-3.5 shrink-0" />
												<span>
													{formatCalendarDate(event.startDate)} – {formatCalendarDate(
														event.endDate
													)}
												</span>
											</p>
										{/if}
										{#if (event.tags?.length ?? 0) > 0}
											<div class="mt-2 flex flex-wrap gap-1">
												{#each event.tags ?? [] as tag (tag.id)}
													<Badge variant="outline" class="font-normal">#{tag.name}</Badge>
												{/each}
											</div>
										{/if}
										{#if event.description}
											<p
												class="mt-2 whitespace-pre-line text-sm leading-relaxed text-foreground/75"
											>
												{event.description}
											</p>
										{/if}
									</article>
								{/each}
							</div>
						</section>
					{/if}

					{#if timedEvents.length > 0}
						<section aria-labelledby="timed-events-heading">
							<div class="mb-3 flex items-center justify-between gap-3">
								<h3
									id="timed-events-heading"
									class="text-xs font-semibold uppercase tracking-wide text-muted-foreground"
								>
									ตามเวลา
								</h3>
								<span class="text-xs text-muted-foreground">{timedEvents.length} รายการ</span>
							</div>

							<div>
								{#each timedEvents as event, eventIndex (event.id)}
									<article class="grid grid-cols-[3.25rem_1rem_minmax(0,1fr)] gap-x-2 pb-4">
										<time
											class="pt-0.5 text-right text-xs font-semibold tabular-nums text-foreground/80"
										>
											{shortTime(event.startTime) || '–'}
										</time>
										<div
											class={cn(
												'relative flex justify-center after:absolute after:top-4 after:w-px after:bg-border',
												eventIndex < timedEvents.length - 1 && 'after:-bottom-4'
											)}
										>
											<span
												class="absolute top-1 z-10 size-2.5 rounded-full ring-4 ring-background"
												style:background-color={event.categoryColor ?? fallbackColor}
											></span>
										</div>
										<div
											class="min-w-0 rounded-lg border border-l-4 bg-card px-3 py-2.5 shadow-sm"
											style:border-left-color={event.categoryColor ?? fallbackColor}
										>
											<div class="flex min-w-0 items-start justify-between gap-2">
												<h4 class="min-w-0 flex-1 font-semibold leading-snug">{event.title}</h4>
												{#if event.categoryName}
													<Badge variant="secondary" class="shrink-0 font-normal">
														{event.categoryName}
													</Badge>
												{/if}
											</div>
											<p class="mt-1.5 flex items-center gap-1.5 text-sm text-muted-foreground">
												<Clock3 class="size-3.5 shrink-0" />
												<span>{timeRange(event)}</span>
											</p>
											{#if event.location}
												<p class="mt-1 flex items-start gap-1.5 text-sm text-muted-foreground">
													<MapPin class="mt-0.5 size-3.5 shrink-0" />
													<span>{event.location}</span>
												</p>
											{/if}
											{#if event.startDate !== event.endDate}
												<p class="mt-1 flex items-start gap-1.5 text-sm text-muted-foreground">
													<CalendarRange class="mt-0.5 size-3.5 shrink-0" />
													<span>
														{formatCalendarDate(event.startDate)} – {formatCalendarDate(
															event.endDate
														)}
													</span>
												</p>
											{/if}
											{#if (event.tags?.length ?? 0) > 0}
												<div class="mt-2 flex flex-wrap gap-1">
													{#each event.tags ?? [] as tag (tag.id)}
														<Badge variant="outline" class="font-normal">#{tag.name}</Badge>
													{/each}
												</div>
											{/if}
											{#if event.description}
												<p
													class="mt-2 whitespace-pre-line text-sm leading-relaxed text-foreground/75"
												>
													{event.description}
												</p>
											{/if}
										</div>
									</article>
								{/each}
							</div>
						</section>
					{/if}
				</div>
			{/if}
		</div>
	</Dialog.Content>
</Dialog.Root>
