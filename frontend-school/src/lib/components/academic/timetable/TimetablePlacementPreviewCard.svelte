<script lang="ts" module>
	import type {
		TimetableLessonColorKey,
		TimetableLessonColors
	} from '#lib/academic/timetable/lesson-color.js';

	export interface TimetablePlacementCard {
		code: string | null;
		title: string;
		colorKey: TimetableLessonColorKey | null;
		colors: TimetableLessonColors;
	}
</script>

<script lang="ts">
	import { timetableLessonColorStyle } from '#lib/academic/timetable/lesson-color.js';
	import '#lib/academic/timetable/lesson-color.css';
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
	data-lesson-color={card.colorKey ?? undefined}
	style={timetableLessonColorStyle(card.colorKey, card.colors)}
	data-placement-state={state}
	aria-hidden="true"
	class={[
		'pointer-events-none absolute inset-1.5 z-30 flex min-w-0 flex-col justify-center overflow-hidden rounded-lg border-2 bg-background/95 p-1.5 text-left shadow-sm backdrop-blur-[1px]',
		card.colorKey && 'timetable-lesson-fill',
		state === 'swap' && 'border-sky-500',
		state === 'blocked' && 'border-destructive',
		state === 'stale' && 'border-orange-500',
		state !== 'swap' && state !== 'blocked' && state !== 'stale' && 'border-emerald-500'
	]}
>
	{#if card.code}
		<p class="truncate text-[10px] leading-[16px] font-semibold">
			{card.code}
		</p>
	{/if}
	<p class="truncate text-[11px] leading-[16px] font-semibold">{card.title}</p>
</article>
