<script lang="ts">
	import {
		BookOpen,
		CalendarDays,
		List,
		MapPin,
		School,
		Shapes,
		Clock,
		ChevronDown,
		X
	} from '@lucide/svelte';
	import { buildTimetableBlockDisplay } from '#lib/academic/timetable/block-display.js';
	import type { TimetableBlock, TimetablePeriodSummary } from '#lib/api/timetable.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';

	type View = 'day' | 'week';
	const uid = $props.id();
	let {
		blocks,
		periods,
		days
	}: {
		blocks: TimetableBlock[];
		periods: TimetablePeriodSummary[];
		days: { value: string; label: string }[];
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
	const kinds = [
		{ value: 'course', label: 'คาบเรียน', icon: BookOpen },
		{ value: 'activity', label: 'กิจกรรม', icon: Shapes },
		{ value: 'structural', label: 'ช่วงส่วนกลาง', icon: Clock }
	] as const;

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
	function tone(kind: TimetableBlock['blockKind']): string {
		if (kind === 'course') return 'border-primary/30 bg-primary/5';
		if (kind === 'activity') return 'border-chart-4/30 bg-chart-4/10';
		return 'border-border bg-muted/60';
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
	{@const display = buildTimetableBlockDisplay(block, 'personal')}
	{@const kind = kinds.find((item) => item.value === block.blockKind)!}
	{@const title =
		block.blockKind === 'structural'
			? (block.title ?? 'กิจกรรม')
			: (block.offeringName ?? block.title ?? 'กิจกรรม')}
	<article
		class={`flex h-full min-w-0 flex-1 flex-col gap-1 rounded-md border p-2 text-sm ${tone(block.blockKind)}`}
		data-timetable-block={block.id}
	>
		<span class="sr-only">{kind.label}</span>
		{#if block.blockKind === 'course' && block.offeringCode}
			<p class="break-words font-semibold">{block.offeringCode}</p>
		{/if}
		<p class="break-words whitespace-pre-line font-medium">{title}</p>
		{#if display.groupLabel || display.roomLabel}
			<div class="mt-auto space-y-1 pt-1 text-xs text-muted-foreground">
				{#if display.groupLabel}<p class="flex items-start gap-1">
						<School class="mt-0.5 size-3 shrink-0" /><span class="min-w-0 break-words"
							>{display.groupLabel}</span
						>
					</p>{/if}
				{#if display.roomLabel}<p class="flex items-start gap-1">
						<MapPin class="mt-0.5 size-3 shrink-0" /><span class="min-w-0 break-words"
							>{display.roomLabel}</span
						>
					</p>{/if}
			</div>
		{/if}
	</article>
{/snippet}

{#snippet weeklyBlockCard(block: TimetableBlock, dayLabel: string, period: TimetablePeriodSummary)}
	{@const display = buildTimetableBlockDisplay(block, 'personal')}
	{@const title =
		block.blockKind === 'structural'
			? (block.title ?? 'กิจกรรม')
			: (block.offeringName ?? block.title ?? 'กิจกรรม')}
	{@const code = block.blockKind === 'course' ? block.offeringCode : null}
	<Popover.Root>
		<Popover.Trigger>
			{#snippet child({ props })}
				<Button
					{...props}
					variant="outline"
					class={`flex h-full w-full min-w-0 flex-1 flex-col items-stretch justify-start gap-1 whitespace-normal p-2 text-left text-xs font-normal shadow-none ${tone(block.blockKind)}`}
					aria-label={`ดูรายละเอียด ${code ? `${code} ` : ''}${title}`}
					data-timetable-block={block.id}
				>
					{#if code}<span class="flex items-start justify-between gap-1"
							><span class="min-w-0 break-words font-semibold">{code}</span><ChevronDown
								class="size-3 shrink-0 text-muted-foreground"
							/></span
						>{/if}
					<span class="line-clamp-2 shrink-0 whitespace-pre-line font-medium">{title}</span>
					{#if display.groupLabel || display.roomLabel}<span
							class="mt-auto space-y-1 pt-1 text-muted-foreground"
						>
							{#if display.groupLabel}<span class="flex items-center gap-1"
									><School class="size-3 shrink-0" /><span class="truncate"
										>{display.groupLabel}</span
									></span
								>{/if}
							{#if display.roomLabel}<span class="flex items-center gap-1"
									><MapPin class="size-3 shrink-0" /><span class="min-w-0 break-words"
										>{display.roomLabel}</span
									></span
								>{/if}
						</span>{/if}
					{#if !code}<ChevronDown class="mt-auto size-3 self-end text-muted-foreground" />{/if}
				</Button>
			{/snippet}
		</Popover.Trigger>
		<Popover.Content
			role="dialog"
			aria-labelledby={`${uid}-details-${block.id}`}
			align="start"
			class="max-h-[min(32rem,calc(100dvh-2rem))] w-80 max-w-[calc(100vw-2rem)] space-y-3 overflow-y-auto"
			data-testid="own-timetable-details"
		>
			<div class="flex items-start justify-between gap-2">
				<div>
					<h3 id={`${uid}-details-${block.id}`} class="text-sm font-semibold">รายละเอียดคาบสอน</h3>
					<p class="mt-1 text-xs text-muted-foreground">
						วัน{dayLabel} · {period.name ?? 'คาบเรียน'}<br />{period.startTime.slice(
							0,
							5
						)}–{period.endTime.slice(0, 5)}
					</p>
				</div>
				<Popover.Close
					>{#snippet child({ props })}<Button
							{...props}
							variant="ghost"
							size="icon-sm"
							aria-label="ปิดรายละเอียดคาบสอน"><X class="size-4" /></Button
						>{/snippet}</Popover.Close
				>
			</div>
			{@render blockCard(block)}
		</Popover.Content>
	</Popover.Root>
{/snippet}

<section class="min-w-0 space-y-3" aria-label="ตารางสอนของฉัน">
	<div class="flex flex-wrap items-center justify-between gap-3">
		<!-- CSS chooses the responsive default before hydration without browser-dependent state. -->
		<div class="md:hidden">{@render viewControls('day')}</div>
		<div class="hidden md:block">{@render viewControls('week')}</div>
		<ul class="flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted-foreground" aria-label="ประเภทคาบ">
			{#each kinds as kind (kind.value)}
				<li class="flex items-center gap-1"><kind.icon class="size-3.5" />{kind.label}</li>
			{/each}
		</ul>
	</div>

	{#if view !== 'week'}
		<div
			class={view === null ? 'space-y-3 md:hidden' : 'space-y-3'}
			data-testid="own-timetable-day"
		>
			<div class="overflow-x-auto">
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
				<h2 class="text-sm font-semibold">วัน{day.label}</h2>
				<ol class="divide-y rounded-lg border bg-card">
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
			{/if}
		</div>
	{/if}

	{#if view !== 'day'}
		<div class={view === null ? 'hidden md:block' : ''} data-testid="own-timetable-week">
			<div
				class="max-h-[70vh] overflow-auto rounded-lg border focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
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
											{#each cellBlocks(option.value, period.id) as block (block.id)}{@render weeklyBlockCard(
													block,
													option.label,
													period
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
