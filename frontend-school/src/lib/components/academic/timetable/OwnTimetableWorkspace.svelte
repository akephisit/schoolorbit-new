<script lang="ts">
	import { MapPin, School } from '@lucide/svelte';
	import { buildStaffOwnTimetableBlockDisplay } from '#lib/academic/timetable/block-display.js';
	import type { TimetableBlock, TimetablePeriodSummary } from '#lib/api/timetable.js';

	let {
		blocks,
		periods,
		days,
		currentStaffId
	}: {
		blocks: TimetableBlock[];
		periods: TimetablePeriodSummary[];
		days: { value: string; label: string }[];
		currentStaffId: string;
	} = $props();

	function cellBlocks(dayValue: string, periodId: string): TimetableBlock[] {
		return blocks.filter(
			(block) => block.dayOfWeek === dayValue && block.bellSchedulePeriodId === periodId
		);
	}
</script>

{#snippet blockEntry(block: TimetableBlock)}
	{@const display = buildStaffOwnTimetableBlockDisplay(block, currentStaffId)}
	{@const title =
		block.blockKind === 'structural'
			? (block.title ?? 'กิจกรรม')
			: (block.offeringName ?? block.title ?? 'กิจกรรม')}
	<div class="min-w-0 px-1 py-1.5 md:px-2" data-timetable-block={block.id}>
		{#if block.blockKind === 'course' && block.offeringCode}
			<p class="font-semibold wrap-break-word">{block.offeringCode}</p>
		{/if}
		<p
			class="font-medium wrap-break-word whitespace-pre-line md:text-[0.8125rem]"
			data-timetable-title
		>
			{title}
		</p>
		{#if display.classLabel || display.roomLabel}
			<div class="mt-0.5 space-y-0.5 text-muted-foreground">
				{#if display.classLabel}
					<p class="flex min-w-0 items-start gap-1">
						<School class="mt-0.5 hidden size-3 shrink-0 md:block" aria-hidden="true" />
						<span class="min-w-0 wrap-break-word" data-timetable-class>{display.classLabel}</span>
					</p>
				{/if}
				{#if display.roomLabel}
					<p class="flex min-w-0 items-start gap-1">
						<MapPin class="mt-0.5 hidden size-3 shrink-0 md:block" aria-hidden="true" />
						<span class="min-w-0 wrap-break-word" data-timetable-room>{display.roomLabel}</span>
					</p>
				{/if}
			</div>
		{/if}
	</div>
{/snippet}

<div class="min-h-0 min-w-0 flex-1" data-testid="own-timetable-week">
	<!-- svelte-ignore a11y_no_noninteractive_tabindex (Native scroll regions need keyboard focus for arrow and page navigation.) -->
	<div
		class="relative max-h-full w-full overflow-auto rounded-lg border focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-hidden"
		tabindex="0"
		role="region"
		aria-label="ตารางสอนรายสัปดาห์ เลื่อนเพื่อดูทุกคาบ"
		data-testid="own-timetable-week-scroll"
	>
		<table
			class="w-full table-fixed border-separate border-spacing-0 bg-card text-left text-xs leading-snug [--day-width:3.5rem] [--period-width:4.5rem] md:[--day-width:4rem] md:[--period-width:7rem]"
			style:--period-count={periods.length}
		>
			<caption class="sr-only">ตารางสอนรายสัปดาห์</caption>
			<thead>
				<tr>
					<th
						scope="col"
						class="day-column sticky top-0 left-0 z-30 border-r border-b bg-muted px-0.5 py-1.5 text-center md:px-1"
					>
						วัน / คาบ
					</th>
					{#each periods as period, index (period.id)}
						<th
							scope="col"
							class="sticky top-0 z-20 border-r border-b bg-muted px-0.5 py-1.5 text-center wrap-break-word last:border-r-0 md:px-1"
						>
							<p class="font-semibold">{period.name ?? `คาบ ${index + 1}`}</p>
							<p class="text-[0.6875rem] font-normal text-muted-foreground tabular-nums md:text-xs">
								{period.startTime.slice(0, 5)}–{period.endTime.slice(0, 5)}
							</p>
						</th>
					{/each}
				</tr>
			</thead>
			<tbody>
				{#each days as day (day.value)}
					<tr>
						<th
							scope="row"
							class="sticky left-0 z-10 border-r border-b bg-muted px-0.5 py-2 text-center wrap-break-word md:px-1"
						>
							{day.label}
						</th>
						{#each periods as period (period.id)}
							{@const entries = cellBlocks(day.value, period.id)}
							<td
								class={[
									'border-r border-b last:border-r-0',
									entries.length > 0 && 'bg-primary/5 align-top'
								]}
							>
								{#if entries.length}
									<div class="divide-y">
										{#each entries as block (block.id)}{@render blockEntry(block)}{/each}
									</div>
								{:else}
									<div class="text-center text-muted-foreground">
										<span aria-hidden="true">–</span>
										<span class="sr-only">ไม่มีคาบสอน</span>
									</div>
								{/if}
							</td>
						{/each}
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
</div>

<style>
	table {
		min-width: calc(var(--day-width) + var(--period-count) * var(--period-width));
	}

	.day-column {
		width: var(--day-width);
	}

	tbody tr:last-child > :is(th, td) {
		border-bottom: 0;
	}
</style>
