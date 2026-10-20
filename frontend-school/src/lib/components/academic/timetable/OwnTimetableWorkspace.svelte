<script lang="ts">
	import { CalendarDays, List, MapPin, School } from '@lucide/svelte';
	import { buildStaffOwnTimetableBlockDisplay } from '#lib/academic/timetable/block-display.js';
	import type { TimetableBlock, TimetablePeriodSummary } from '#lib/api/timetable.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';

	type View = 'day' | 'week';
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
	let selectedView = $state<View | null>(null);
	let selectedDay = $state('');
	const view = $derived(selectedView);
	const day = $derived(days.find((item) => item.value === selectedDay) ?? days[0]);
	const shortDays: Record<string, string> = {
		MON: 'จ.',
		TUE: 'อ.',
		WED: 'พ.',
		THU: 'พฤ.',
		FRI: 'ศ.',
		SAT: 'ส.',
		SUN: 'อา.'
	};

	function changeView(value: string): void {
		if (value === 'day' || value === 'week') selectedView = value;
	}
	function changeDay(value: string): void {
		if (value) selectedDay = value;
	}
	function cellBlocks(dayValue: string, periodId: string): TimetableBlock[] {
		return blocks.filter(
			(block) => block.dayOfWeek === dayValue && block.bellSchedulePeriodId === periodId
		);
	}
</script>

{#snippet viewControls(defaultView: View)}
	<ToggleGroup.Root
		type="single"
		bind:value={() => view ?? defaultView, changeView}
		aria-label="มุมมองตารางสอน"
	>
		<ToggleGroup.Item value="day"><List class="size-4" />รายวัน</ToggleGroup.Item>
		<ToggleGroup.Item value="week"><CalendarDays class="size-4" />รายสัปดาห์</ToggleGroup.Item>
	</ToggleGroup.Root>
{/snippet}

{#snippet blockCard(block: TimetableBlock)}
	{@const display = buildStaffOwnTimetableBlockDisplay(block, currentStaffId)}
	{@const title =
		block.blockKind === 'structural'
			? (block.title ?? 'กิจกรรม')
			: (block.offeringName ?? block.title ?? 'กิจกรรม')}
	<article
		class="flex h-full min-w-0 flex-1 flex-col gap-1 rounded-md border border-primary/30 bg-primary/5 p-2 text-sm"
		data-timetable-block={block.id}
	>
		{#if block.blockKind === 'course' && block.offeringCode}
			<p class="break-words font-semibold">{block.offeringCode}</p>
		{/if}
		<p class="break-words whitespace-pre-line font-medium" data-timetable-title>{title}</p>
		{#if display.classLabel || display.roomLabel}
			<div class="mt-auto space-y-1 pt-1 text-xs text-muted-foreground">
				{#if display.classLabel}<p class="flex items-start gap-1">
						<School class="mt-0.5 size-3 shrink-0" /><span
							class="min-w-0 break-words"
							data-timetable-class>{display.classLabel}</span
						>
					</p>{/if}
				{#if display.roomLabel}<p class="flex items-start gap-1">
						<MapPin class="mt-0.5 size-3 shrink-0" /><span
							class="min-w-0 break-words"
							data-timetable-room>{display.roomLabel}</span
						>
					</p>{/if}
			</div>
		{/if}
	</article>
{/snippet}

<section class="flex min-h-0 min-w-0 flex-1 flex-col gap-3" aria-label="ตารางสอนของฉัน">
	<div class="flex shrink-0 flex-wrap items-center gap-3">
		<!-- CSS chooses the responsive default before hydration without browser-dependent state. -->
		<div class="md:hidden">{@render viewControls('day')}</div>
		<div class="hidden md:block">{@render viewControls('week')}</div>
	</div>

	{#if view !== 'week'}
		<div
			class={view === null
				? 'flex min-h-0 flex-1 flex-col gap-3 md:hidden'
				: 'flex min-h-0 flex-1 flex-col gap-3'}
			data-testid="own-timetable-day"
		>
			<div class="shrink-0 overflow-x-auto">
				<ToggleGroup.Root
					type="single"
					class="w-max"
					bind:value={() => day?.value ?? '', changeDay}
					aria-label="วันในตารางสอน"
				>
					{#each days as option (option.value)}
						<ToggleGroup.Item value={option.value} aria-label={`วัน${option.label}`}
							>{shortDays[option.value] ?? option.label}</ToggleGroup.Item
						>
					{/each}
				</ToggleGroup.Root>
			</div>
			{#if day}
				<h2 class="shrink-0 text-sm font-semibold">วัน{day.label}</h2>
				<!-- svelte-ignore a11y_no_noninteractive_tabindex (Native scroll regions need keyboard focus for arrow and page navigation.) -->
				<div
					class="min-h-0 overflow-y-auto rounded-lg border"
					tabindex="0"
					role="region"
					aria-label="คาบสอนรายวัน เลื่อนเพื่อดูทุกคาบ"
					data-testid="own-timetable-day-scroll"
				>
					<ol class="grid auto-rows-fr divide-y bg-card">
						{#each periods as period, index (period.id)}
							{@const entries = cellBlocks(day.value, period.id)}
							<li
								class="grid grid-cols-[5.5rem_minmax(0,1fr)] gap-2 p-2 sm:grid-cols-[7rem_minmax(0,1fr)]"
							>
								<div class="py-1 text-xs">
									<p class="font-semibold">{period.name ?? `คาบ ${index + 1}`}</p>
									<p class="mt-1 text-muted-foreground tabular-nums">
										{period.startTime.slice(0, 5)}–{period.endTime.slice(0, 5)}
									</p>
								</div>
								<div class="flex min-w-0 flex-col gap-1">
									{#each entries as block (block.id)}{@render blockCard(block)}{:else}<p
											class="px-2 py-1 text-sm text-muted-foreground"
										>
											ไม่มีคาบสอน
										</p>{/each}
								</div>
							</li>
						{/each}
					</ol>
				</div>
			{/if}
		</div>
	{/if}

	{#if view !== 'day'}
		<div
			class={view === null ? 'min-h-0 min-w-0 flex-1 hidden md:block' : 'min-h-0 min-w-0 flex-1'}
			data-testid="own-timetable-week"
		>
			<!-- svelte-ignore a11y_no_noninteractive_tabindex (Native scroll regions need keyboard focus for arrow and page navigation.) -->
			<div
				class="relative max-h-full w-full min-w-0 overflow-auto rounded-lg border focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
				tabindex="0"
				role="region"
				aria-label="ตารางสอนรายสัปดาห์ เลื่อนเพื่อดูทุกคาบ"
				data-testid="own-timetable-week-scroll"
			>
				<table
					class="w-full table-fixed border-separate border-spacing-0 text-left text-xs"
					style={`min-width: ${80 + periods.length * 132}px`}
				>
					<caption class="sr-only">ตารางสอนรายสัปดาห์</caption>
					<thead>
						<tr>
							<th
								scope="col"
								class="sticky top-0 left-0 z-30 w-20 border-r border-b bg-muted p-2 text-center"
								>วัน / คาบ</th
							>
							{#each periods as period, index (period.id)}
								<th
									scope="col"
									class="sticky top-0 z-20 border-r border-b bg-muted p-2 text-center last:border-r-0"
									><p class="font-semibold">{period.name ?? `คาบ ${index + 1}`}</p>
									<p class="mt-1 font-normal text-muted-foreground tabular-nums">
										{period.startTime.slice(0, 5)}–{period.endTime.slice(0, 5)}
									</p></th
								>
							{/each}
						</tr>
					</thead>
					<tbody>
						{#each days as option (option.value)}
							<tr>
								<th scope="row" class="sticky left-0 z-10 border-r border-b bg-card p-2 text-center"
									>{option.label}</th
								>
								{#each periods as period (period.id)}
									<td class="h-px border-r border-b p-1 align-top last:border-r-0"
										><div class="flex h-full min-h-20 flex-col gap-1">
											{#each cellBlocks(option.value, period.id) as block (block.id)}{@render blockCard(
													block
												)}{:else}<span class="sr-only">ไม่มีคาบสอน</span>{/each}
										</div></td
									>
								{/each}
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		</div>
	{/if}
</section>
