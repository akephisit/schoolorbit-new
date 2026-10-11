<script lang="ts">
	import { periodsFromTimetableBlocks, type TimetableBlock } from '#lib/api/timetable.js';
	import { buildTimetableBlockDisplay } from '#lib/academic/timetable/block-display.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { MapPin, Users } from '@lucide/svelte';
	let { blocks }: { blocks: TimetableBlock[] } = $props();
	const dayOptions = [
		{ value: 'MON', label: 'จันทร์' },
		{ value: 'TUE', label: 'อังคาร' },
		{ value: 'WED', label: 'พุธ' },
		{ value: 'THU', label: 'พฤหัสบดี' },
		{ value: 'FRI', label: 'ศุกร์' },
		{ value: 'SAT', label: 'เสาร์' },
		{ value: 'SUN', label: 'อาทิตย์' }
	];
	const periods = $derived(periodsFromTimetableBlocks(blocks));
	const days = $derived(
		dayOptions.filter((day) => blocks.some((block) => block.dayOfWeek === day.value))
	);
	function cell(day: string, period: string) {
		return blocks.filter(
			(block) => block.dayOfWeek === day && block.bellSchedulePeriodId === period
		);
	}
</script>

{#snippet lesson(block: TimetableBlock)}
	{@const display = buildTimetableBlockDisplay(block, 'personal')}
	<article
		data-personal-timetable-block={block.id}
		data-block-kind={block.blockKind}
		class="min-w-0 space-y-2 rounded-lg border bg-card p-3 text-left text-sm whitespace-normal shadow-xs"
	>
		{#if block.blockKind === 'course' && block.offeringCode}<p
				class="break-words font-semibold text-primary"
			>
				{block.offeringCode}
			</p>{/if}
		<h3 class="break-words font-semibold">{block.offeringName ?? block.title ?? 'กิจกรรม'}</h3>
		{#if display.contextLabel}<Badge variant="secondary">{display.contextLabel}</Badge>{/if}
		{#if display.groupLabel}<p class="break-words text-xs text-muted-foreground">
				{display.groupLabel}
			</p>{/if}
		{#if display.teacherLabel && !display.shared}<p
				class="flex items-start gap-1.5 text-xs text-muted-foreground"
			>
				<Users class="mt-0.5 size-3 shrink-0" /><span class="break-words"
					>{display.teacherLabel}</span
				>
			</p>{/if}
		{#if display.roomLabel}<p class="flex items-start gap-1.5 text-xs text-muted-foreground">
				<MapPin class="mt-0.5 size-3 shrink-0" /><span class="break-words">{display.roomLabel}</span
				>
			</p>{/if}
	</article>
{/snippet}

<div class="min-w-0 md:rounded-xl md:border" data-testid="personal-timetable">
	<Table.Root
		class="block w-full min-w-0 md:table md:table-fixed md:min-w-(--timetable-width)"
		style={`--timetable-width: ${100 + periods.length * 180}px`}
	>
		<Table.Caption class="sr-only">ตารางเรียนรายสัปดาห์</Table.Caption>
		<Table.Header class="hidden md:table-header-group"
			><Table.Row class="bg-muted/40"
				><Table.Head class="w-24 border-r text-center">วัน / คาบ</Table.Head>
				{#each periods as period, index (period.id)}<Table.Head class="border-r py-3 text-center"
						><p class="font-semibold text-foreground">{period.name ?? `คาบ ${index + 1}`}</p>
						<p class="mt-1 font-normal">
							{period.startTime.slice(0, 5)}–{period.endTime.slice(0, 5)}
						</p></Table.Head
					>{/each}
			</Table.Row></Table.Header
		>
		<Table.Body class="block space-y-6 md:table-row-group md:space-y-0">
			{#each days as day (day.value)}<Table.Row
					class="block space-y-3 border-0 hover:bg-transparent md:table-row md:space-y-0 md:border-b"
				>
					<Table.Head
						scope="row"
						class="block h-auto p-0 font-semibold text-foreground md:table-cell md:border-r md:bg-muted/20 md:p-2 md:text-center"
						>{day.label}</Table.Head
					>
					{#each periods as period (period.id)}{@const cellBlocks = cell(day.value, period.id)}
						<Table.Cell
							class={[
								cellBlocks.length ? 'block' : 'hidden',
								'p-0 align-top md:table-cell md:border-r md:p-2'
							]}
						>
							<div class="space-y-3 md:space-y-2">
								{#each cellBlocks as block (block.id)}<div
										class="grid grid-cols-[5rem_minmax(0,1fr)] gap-3 md:block"
									>
										<p class="pt-3 text-xs text-muted-foreground md:hidden">
											{block.startTime.slice(0, 5)}–{block.endTime.slice(0, 5)}
										</p>
										{@render lesson(block)}
									</div>{/each}
							</div>
						</Table.Cell>
					{/each}
				</Table.Row>{/each}
		</Table.Body>
	</Table.Root>
</div>
