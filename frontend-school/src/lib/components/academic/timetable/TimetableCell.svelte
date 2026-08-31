<script lang="ts" module>
	export type TimetableCellState =
		| 'neutral'
		| 'dragging'
		| 'move'
		| 'swap'
		| 'blocked'
		| 'saving'
		| 'stale';
</script>

<script lang="ts">
	import type { Snippet } from 'svelte';
	import {
		AlertTriangle,
		ArrowRight,
		ArrowRightLeft,
		Grip,
		LoaderCircle,
		RefreshCw
	} from 'lucide-svelte';

	let {
		dayLabel,
		periodLabel,
		state = 'neutral',
		disabled = false,
		onDropIntent,
		onHoverIntent,
		children
	}: {
		dayLabel: string;
		periodLabel: string;
		state?: TimetableCellState;
		disabled?: boolean;
		onDropIntent?: (event: DragEvent) => void;
		onHoverIntent?: () => void;
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
</script>

<td
	class={[
		'relative min-w-44 border-b border-r p-1.5 align-top transition-colors',
		state === 'dragging' && 'bg-primary/5 ring-2 ring-inset ring-primary/35',
		state === 'move' && 'bg-emerald-50 ring-2 ring-inset ring-emerald-500 dark:bg-emerald-950/25',
		state === 'swap' && 'bg-sky-50 ring-2 ring-inset ring-sky-500 dark:bg-sky-950/25',
		state === 'blocked' && 'bg-destructive/5 ring-2 ring-inset ring-destructive/60',
		state === 'saving' && 'animate-pulse bg-amber-50 dark:bg-amber-950/20',
		state === 'stale' && 'bg-orange-50 ring-2 ring-inset ring-orange-500 dark:bg-orange-950/20'
	]}
	aria-label={`${dayLabel} ${periodLabel} — ${stateLabel}`}
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
	{#if state !== 'neutral'}
		<div
			class={[
				'mb-1.5 flex items-center gap-1 rounded-md border px-2 py-1 text-[0.68rem] font-medium',
				state === 'dragging' && 'border-primary/30 bg-primary/5 text-primary',
				state === 'move' && 'border-emerald-300 bg-emerald-50 text-emerald-800',
				state === 'swap' && 'border-sky-300 bg-sky-50 text-sky-800',
				state === 'blocked' && 'border-destructive/40 bg-destructive/5 text-destructive',
				state === 'saving' && 'border-amber-300 bg-amber-50 text-amber-800',
				state === 'stale' && 'border-orange-300 bg-orange-50 text-orange-800'
			]}
		>
			{#if state === 'dragging'}
				<Grip class="size-3" />
			{:else if state === 'move'}
				<ArrowRight class="size-3" />
			{:else if state === 'swap'}
				<ArrowRightLeft class="size-3" />
			{:else if state === 'blocked'}
				<AlertTriangle class="size-3" />
			{:else if state === 'saving'}
				<LoaderCircle class="size-3 animate-spin" />
			{:else}
				<RefreshCw class="size-3" />
			{/if}
			{stateLabel}
		</div>
	{/if}
	<div class="min-h-28 space-y-1.5">{@render children?.()}</div>
</td>
