<script lang="ts" module>
	export type TimetableCellState =
		'neutral' | 'dragging' | 'move' | 'swap' | 'blocked' | 'saving' | 'stale';
</script>

<script lang="ts">
	import type { Snippet } from 'svelte';
	import { LoaderCircle } from '@lucide/svelte';
	import TimetablePlacementPreviewCard, {
		type TimetablePlacementCard
	} from './TimetablePlacementPreviewCard.svelte';

	let {
		dayOfWeek,
		periodId,
		dayLabel,
		periodLabel,
		state = 'neutral',
		disabled = false,
		onDropIntent,
		onHoverIntent,
		onActivateIntent,
		placementCard = null,
		pendingRemoval = false,
		children
	}: {
		dayOfWeek: string;
		periodId: string;
		dayLabel: string;
		periodLabel: string;
		state?: TimetableCellState;
		disabled?: boolean;
		onDropIntent?: (event: DragEvent) => void;
		onHoverIntent?: () => void;
		onActivateIntent?: () => void;
		placementCard?: TimetablePlacementCard | null;
		pendingRemoval?: boolean;
		children?: Snippet;
	} = $props();

	const stateLabels: Record<TimetableCellState, string> = {
		neutral: 'ว่าง',
		dragging: 'กำลังเลือกตำแหน่ง',
		move: 'วางได้',
		swap: 'สลับได้',
		blocked: 'วางไม่ได้',
		saving: 'กำลังบันทึก',
		stale: 'ข้อมูลเปลี่ยนแล้ว'
	};
	const stateLabel = $derived(stateLabels[state]);
	const canActivate = $derived(!disabled && (state === 'move' || state === 'swap'));
	const actionLabel = $derived(state === 'swap' ? 'สลับกับคาบนี้' : 'วางคาบที่นี่');
</script>

<td
	class={[
		'relative border-b border-r p-1.5 align-top transition-colors',
		state === 'dragging' && 'bg-primary/5 ring-2 ring-inset ring-primary/35',
		state === 'move' && 'bg-emerald-50 ring-2 ring-inset ring-emerald-500 dark:bg-emerald-950/25',
		state === 'swap' && 'bg-sky-50 ring-2 ring-inset ring-sky-500 dark:bg-sky-950/25',
		state === 'blocked' && 'bg-destructive/5 ring-2 ring-inset ring-destructive/60',
		state === 'saving' && 'animate-pulse bg-amber-50 dark:bg-amber-950/20',
		state === 'stale' && 'bg-orange-50 ring-2 ring-inset ring-orange-500 dark:bg-orange-950/20'
	]}
	aria-label={`${dayLabel} ${periodLabel} — ${stateLabel}`}
	data-timetable-day={dayOfWeek}
	data-timetable-period-id={periodId}
	data-state={state}
	ondragover={(event) => {
		if (disabled) return;
		event.preventDefault();
		onHoverIntent?.();
	}}
	ondrop={(event) => {
		if (disabled) return;
		event.preventDefault();
		onDropIntent?.(event);
	}}
>
	<span class="sr-only">{stateLabel}</span>
	<div class="flex min-h-24 min-w-0 flex-col">
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
	{#if placementCard}
		<TimetablePlacementPreviewCard card={placementCard} {state} />
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
