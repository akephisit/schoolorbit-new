<script lang="ts" module>
	export interface TimetablePlacementCard {
		code: string | null;
		title: string;
	}
</script>

<script lang="ts">
	let {
		card,
		state
	}: {
		card: TimetablePlacementCard;
		state: 'neutral' | 'dragging' | 'move' | 'swap' | 'blocked' | 'saving' | 'stale';
	} = $props();
</script>

<article
	data-timetable-placement-preview
	data-placement-state={state}
	aria-hidden="true"
	class={[
		'pointer-events-none absolute inset-1.5 z-30 flex min-w-0 flex-col justify-center overflow-hidden rounded-lg border-2 bg-background/95 p-1.5 text-left shadow-sm backdrop-blur-[1px]',
		state === 'swap' && 'border-sky-500 bg-sky-50/95 dark:bg-sky-950/90',
		state === 'blocked' && 'border-destructive bg-destructive/10',
		state !== 'swap' &&
			state !== 'blocked' &&
			'border-emerald-500 bg-emerald-50/95 dark:bg-emerald-950/90'
	]}
>
	{#if card.code}
		<p class="truncate font-mono text-[8px] leading-[14px] font-semibold text-primary">
			{card.code}
		</p>
	{/if}
	<p class="truncate text-[9px] leading-[14px] font-semibold">{card.title}</p>
</article>
