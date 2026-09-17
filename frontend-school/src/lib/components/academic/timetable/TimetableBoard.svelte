<script lang="ts">
	import type {
		TimetableBoardRow,
		TimetableBoardState,
		TimetableBoardView
	} from '$lib/academic/timetable/board-state';
	import { blocksForTimetableCell } from '$lib/academic/timetable/board-state';
	import type { TimetableBlock } from '$lib/api/timetable';
	import { buildSchedulerTargetLabel } from '$lib/academic/timetable/block-display';

	import TimetableCell, { type TimetableCellState } from './TimetableCell.svelte';
	import TimetableLessonCard from './TimetableLessonCard.svelte';
	import type { TimetablePlacementCard } from './TimetablePlacementPreviewCard.svelte';

	type Day = { id: string; label: string; shortLabel: string };

	let {
		state,
		view,
		row,
		selectedBlockId = null,
		canEdit = false,
		cellState,
		onDropIntent,
		onActivateIntent,
		onHoverIntent,
		placementPreview,
		onSelectBlock,
		onDragStart,
		onCancelDrag,
		onRemoveBlock
	}: {
		state: TimetableBoardState;
		view: TimetableBoardView;
		row: TimetableBoardRow;
		selectedBlockId?: string | null;
		canEdit?: boolean;
		cellState?: (dayOfWeek: string, periodId: string) => TimetableCellState;
		onDropIntent?: (dayOfWeek: string, periodId: string) => void;
		onActivateIntent?: (dayOfWeek: string, periodId: string) => void;
		onHoverIntent?: (dayOfWeek: string, periodId: string) => void;
		placementPreview?: (dayOfWeek: string, periodId: string) => TimetablePlacementCard | null;
		onSelectBlock?: (block: TimetableBlock) => void;
		onDragStart?: (block: TimetableBlock, event: DragEvent) => void;
		onCancelDrag?: () => void;
		onRemoveBlock?: (block: TimetableBlock) => void;
	} = $props();

	const days: Day[] = [
		{ id: 'MON', label: 'วันจันทร์', shortLabel: 'จ.' },
		{ id: 'TUE', label: 'วันอังคาร', shortLabel: 'อ.' },
		{ id: 'WED', label: 'วันพุธ', shortLabel: 'พ.' },
		{ id: 'THU', label: 'วันพฤหัสบดี', shortLabel: 'พฤ.' },
		{ id: 'FRI', label: 'วันศุกร์', shortLabel: 'ศ.' }
	];
	const homeroomNamesById = $derived(
		new Map(state.workspace.homerooms.map((homeroom) => [homeroom.id, homeroom.name]))
	);

	function periodLabel(period: TimetableBoardState['workspace']['bellPeriods'][number]): string {
		return period.name ?? `คาบที่ ${period.orderIndex}`;
	}

	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Escape') onCancelDrag?.();
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<section
	class="min-w-0 overflow-hidden rounded-xl border bg-background"
	aria-label={`ตารางของ ${row.label}`}
>
	<div class="flex items-center justify-between border-b bg-muted/20 px-4 py-3">
		<div>
			<p class="font-mono text-xs font-semibold text-primary">{row.code}</p>
			<h2 class="font-semibold">{row.label}</h2>
		</div>
		<p class="text-xs text-muted-foreground">ลากคาบไปยังช่องใหม่ · ครั้งละ 1 คาบ</p>
	</div>
	<div class="overflow-x-auto" data-timetable-scroll-container>
		<table class="w-full min-w-[70rem] table-fixed border-collapse text-left">
			<thead>
				<tr class="bg-muted/35">
					<th
						class="sticky left-0 z-10 w-14 border-b border-r bg-muted/70 px-1.5 py-2 text-center text-xs font-semibold"
						>วัน / คาบ</th
					>
					{#each state.workspace.bellPeriods as period (period.id)}
						<th class="border-b border-r px-1.5 py-2 text-center text-xs font-semibold">
							<p>{periodLabel(period)}</p>
							<p class="mt-1 font-mono text-[0.65rem] font-normal text-muted-foreground">
								{period.startTime.slice(0, 5)}–{period.endTime.slice(0, 5)}
							</p>
						</th>
					{/each}
				</tr>
			</thead>
			<tbody>
				{#each days as day (day.id)}
					<tr>
						<th
							class="sticky left-0 z-10 border-b border-r bg-background px-1.5 py-3 text-center align-top"
							aria-label={day.label}
						>
							<p class="text-xs font-semibold">{day.shortLabel}</p>
						</th>
						{#each state.workspace.bellPeriods as period (period.id)}
							{@const currentCellState = cellState?.(day.id, period.id) ?? 'neutral'}
							{@const blocks = blocksForTimetableCell(state, {
								view,
								rowId: row.id,
								dayOfWeek: day.id,
								bellSchedulePeriodId: period.id
							})}
							<TimetableCell
								dayOfWeek={day.id}
								periodId={period.id}
								dayLabel={day.label}
								periodLabel={periodLabel(period)}
								state={currentCellState}
								disabled={!canEdit}
								placementCard={placementPreview?.(day.id, period.id) ?? null}
								onHoverIntent={() => onHoverIntent?.(day.id, period.id)}
								onDropIntent={() => onDropIntent?.(day.id, period.id)}
								onActivateIntent={() => onActivateIntent?.(day.id, period.id)}
							>
								{#each blocks as block (`${block.id}:${row.id}`)}
									<TimetableLessonCard
										{block}
										rowId={row.id}
										targetLabel={buildSchedulerTargetLabel(block, view, homeroomNamesById)}
										showTeacher={view !== 'teacher'}
										selected={selectedBlockId === block.id}
										{canEdit}
										onSelect={onSelectBlock}
										{onDragStart}
										onDragEnd={onCancelDrag}
										onRemove={onRemoveBlock}
									/>
								{/each}
							</TimetableCell>
						{/each}
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
</section>
