<script lang="ts" module>
	export type TimetableCellState =
		'neutral' | 'dragging' | 'move' | 'swap' | 'blocked' | 'saving' | 'stale';
</script>

<script lang="ts">
	import type { Snippet } from 'svelte';
	import TimetablePlacementReasons from './TimetablePlacementReasons.svelte';
	import type { TimetablePlacementReasons as PlacementReasons } from '#lib/academic/timetable/placement-reasons.js';
	import { LoaderCircle, LockKeyhole } from '@lucide/svelte';
	import TimetablePlacementPreviewCard, {
		type TimetablePlacementCard
	} from './TimetablePlacementPreviewCard.svelte';

	let {
		dayOfWeek,
		periodId,
		dayLabel,
		periodLabel,
		rowId = null,
		state: cellState = 'neutral',
		disabled = false,
		lockedBy = null,
		onDropIntent,
		onHoverIntent,
		onActivateIntent,
		placementCard = null,
		placementReasons = null,
		pendingRemoval = false,
		children
	}: {
		dayOfWeek: string;
		periodId: string;
		dayLabel: string;
		periodLabel: string;
		rowId?: string | null;
		state?: TimetableCellState;
		disabled?: boolean;
		lockedBy?: string | null;
		onDropIntent?: (event: DragEvent) => void;
		onHoverIntent?: () => void;
		onActivateIntent?: () => void;
		placementCard?: TimetablePlacementCard | null;
		placementReasons?: PlacementReasons | null;
		pendingRemoval?: boolean;
		children?: Snippet;
	} = $props();

	let dragActive = $state(false);
	const stateLabels: Record<TimetableCellState, string> = {
		neutral: 'ว่าง',
		dragging: 'กำลังเลือกตำแหน่ง',
		move: 'วางได้',
		swap: 'สลับได้',
		blocked: 'วางไม่ได้',
		saving: 'กำลังบันทึก',
		stale: 'ข้อมูลเปลี่ยนแล้ว'
	};
	const stateLabel = $derived(lockedBy ? `${lockedBy} กำลังใช้ช่องนี้` : stateLabels[cellState]);
	const unavailable = $derived(disabled || Boolean(lockedBy));
	const canActivate = $derived(!unavailable && (cellState === 'move' || cellState === 'swap'));
	const actionLabel = $derived(cellState === 'swap' ? 'สลับกับคาบนี้' : 'วางคาบที่นี่');
</script>

<td
	class={[
		'relative h-px border-b border-r p-1.5 align-top transition-colors',
		lockedBy && 'bg-muted/60 ring-2 ring-inset ring-muted-foreground/30',
		cellState === 'dragging' && 'bg-primary/5 ring-2 ring-inset ring-primary/35',
		cellState === 'move' &&
			'bg-emerald-50 ring-2 ring-inset ring-emerald-500 dark:bg-emerald-950/25',
		cellState === 'swap' && 'bg-sky-50 ring-2 ring-inset ring-sky-500 dark:bg-sky-950/25',
		cellState === 'blocked' && 'bg-destructive/5 ring-2 ring-inset ring-destructive/60',
		cellState === 'saving' && 'animate-pulse bg-amber-50 dark:bg-amber-950/20',
		cellState === 'stale' && 'bg-orange-50 ring-2 ring-inset ring-orange-500 dark:bg-orange-950/20'
	]}
	aria-label={`${dayLabel} ${periodLabel} — ${stateLabel}`}
	data-timetable-day={dayOfWeek}
	data-timetable-anchor="cell"
	data-timetable-owner-id={rowId ?? undefined}
	data-timetable-period-id={periodId}
	data-state={cellState}
	data-timetable-locked-by={lockedBy ?? undefined}
	ondragover={(event) => {
		if (unavailable) return;
		event.preventDefault();
		dragActive = true;
		onHoverIntent?.();
	}}
	ondragleave={(event) => {
		if (
			!(event.relatedTarget instanceof Node) ||
			!event.currentTarget.contains(event.relatedTarget)
		)
			dragActive = false;
	}}
	ondrop={(event) => {
		dragActive = false;
		if (unavailable) return;
		event.preventDefault();
		onDropIntent?.(event);
	}}
>
	{#if lockedBy}
		<span
			class="pointer-events-none absolute right-1.5 top-1.5 z-40 inline-flex max-w-[calc(100%-0.75rem)] items-center gap-1 rounded border bg-background/95 px-1.5 py-0.5 text-[10px] shadow-sm"
			title={stateLabel}
		>
			<LockKeyhole class="size-3 shrink-0" /><span class="truncate">{lockedBy}</span>
		</span>
	{/if}
	<span class="sr-only">{stateLabel}</span>
	<div class="flex h-full min-h-24 min-w-0 flex-col">
		<div class="grid min-h-0 min-w-0 flex-1 auto-rows-fr gap-1.5">
			{@render children?.()}
		</div>
	</div>
	{#if canActivate}
		<button
			type="button"
			class="absolute inset-0 z-20 cursor-copy border-0 bg-transparent p-0 outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-inset"
			aria-label={actionLabel}
			onclick={onActivateIntent}
		>
			<span class="sr-only">{actionLabel}</span>
		</button>
	{/if}
	{#if cellState === 'blocked' && placementReasons}
		<TimetablePlacementReasons reasons={placementReasons} active={dragActive} />
	{/if}
	{#if placementCard && cellState !== 'blocked'}
		<TimetablePlacementPreviewCard card={placementCard} state={cellState} />
	{/if}
	{#if pendingRemoval}
		<span
			class="pointer-events-none absolute right-2 top-2 z-30 grid size-6 place-items-center rounded-full border bg-background/95 text-primary shadow-sm"
			aria-label="กำลังลบคาบ"
		>
			<LoaderCircle class="size-3.5 animate-spin" />
		</span>
	{/if}
</td>
