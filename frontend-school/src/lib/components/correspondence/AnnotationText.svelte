<script lang="ts">
	import { fitAnnotationText, type TextMeasure } from '#lib/correspondence/text-layout.js';
	let {
		text,
		x,
		y,
		width,
		height,
		color,
		measure
	}: {
		text: string;
		x: number;
		y: number;
		width: number;
		height: number;
		color: string;
		measure: TextMeasure;
	} = $props();
	const layout = $derived(fitAnnotationText(text, width, height, measure));
</script>

<text {x} font-size={layout.size} font-family="CorrespondenceSarabun" fill={color}>
	{#each layout.lines as line, index (index)}<tspan
			{x}
			y={y + layout.size * 1.1 + index * layout.size * 1.3}>{line}</tspan
		>{/each}
</text>
