<script lang="ts">
	import type {
		TimetableBoardRow,
		TimetableBoardState,
		TimetableBoardView
	} from '$lib/academic/timetable/board-state';
	import { entriesForTimetableCell } from '$lib/academic/timetable/board-state';
	import type { TimetableEntry } from '$lib/api/timetable';

	import TimetableCell, { type TimetableCellState } from './TimetableCell.svelte';
	import TimetableLessonCard from './TimetableLessonCard.svelte';

	type Day = { id: string; label: string };

	let {
		state,
		view,
		row,
		selectedEntryId = null,
		canEdit = false,
		cellState,
		onDropIntent,
		onActivateIntent,
		onHoverIntent,
		onSelectEntry,
		onDragStart,
		onCancelDrag,
		onMoveEntry,
		onEditEntry,
		onRemoveEntry
	}: {
		state: TimetableBoardState;
		view: TimetableBoardView;
		row: TimetableBoardRow;
		selectedEntryId?: string | null;
		canEdit?: boolean;
		cellState?: (dayOfWeek: string, periodId: string) => TimetableCellState;
		onDropIntent?: (dayOfWeek: string, periodId: string) => void;
		onActivateIntent?: (dayOfWeek: string, periodId: string) => void;
		onHoverIntent?: (dayOfWeek: string, periodId: string) => void;
		onSelectEntry?: (entry: TimetableEntry) => void;
		onDragStart?: (entry: TimetableEntry, event: DragEvent) => void;
		onCancelDrag?: () => void;
		onMoveEntry?: (entry: TimetableEntry) => void;
		onEditEntry?: (entry: TimetableEntry) => void;
		onRemoveEntry?: (entry: TimetableEntry) => void;
	} = $props();

	const days: Day[] = [
		{ id: 'MON', label: 'วันจันทร์' },
		{ id: 'TUE', label: 'วันอังคาร' },
		{ id: 'WED', label: 'วันพุธ' },
		{ id: 'THU', label: 'วันพฤหัสบดี' },
		{ id: 'FRI', label: 'วันศุกร์' }
	];

	function periodLabel(period: TimetableBoardState['workspace']['bellPeriods'][number]): string {
		return period.name ?? `คาบที่ ${period.orderIndex}`;
	}

	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Escape') onCancelDrag?.();
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<section
	class="overflow-hidden rounded-xl border bg-background"
	aria-label={`ตารางของ ${row.label}`}
>
	<div class="flex items-center justify-between border-b bg-muted/20 px-4 py-3">
		<div>
			<p class="font-mono text-xs font-semibold text-primary">{row.code}</p>
			<h2 class="font-semibold">{row.label}</h2>
		</div>
		<p class="text-xs text-muted-foreground">ลากหรือกด “ย้ายคาบ” · ครั้งละ 1 คาบ</p>
	</div>
	<div class="overflow-x-auto">
		<table class="w-full border-collapse text-left">
			<thead>
				<tr class="bg-muted/35">
					<th
						class="sticky left-0 z-10 w-28 min-w-28 border-b border-r bg-muted/70 px-3 py-2 text-xs font-semibold"
						>วัน / คาบ</th
					>
					{#each state.workspace.bellPeriods as period (period.id)}
						<th class="min-w-44 border-b border-r px-3 py-2 text-center text-xs font-semibold">
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
						<th class="sticky left-0 z-10 border-b border-r bg-background px-3 py-3 align-top">
							<p class="text-xs font-semibold">{day.label}</p>
						</th>
						{#each state.workspace.bellPeriods as period (period.id)}
							{@const entries = entriesForTimetableCell(state, {
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
								state={cellState?.(day.id, period.id) ?? 'neutral'}
								disabled={!canEdit}
								onHoverIntent={() => onHoverIntent?.(day.id, period.id)}
								onDropIntent={() => onDropIntent?.(day.id, period.id)}
								onActivateIntent={() => onActivateIntent?.(day.id, period.id)}
							>
								{#each entries as entry (`${entry.id}:${row.id}`)}
									<TimetableLessonCard
										{entry}
										rowId={row.id}
										selected={selectedEntryId === entry.id}
										{canEdit}
										onSelect={onSelectEntry}
										{onDragStart}
										onDragEnd={onCancelDrag}
										onMove={onMoveEntry}
										onEdit={onEditEntry}
										onRemove={onRemoveEntry}
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
