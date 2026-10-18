<script lang="ts">
	import { fitAnnotationBox, type TextMeasure } from '#lib/correspondence/text-layout.js';
	let {
		text,
		x,
		y,
		width,
		height,
		color,
		measure,
		border = false,
		textLayoutVersion
	}: {
		text: string;
		x: number;
		y: number;
		width: number;
		height: number;
		color: string;
		measure: TextMeasure;
		border?: boolean;
		textLayoutVersion?: number | null;
	} = $props();
	const layout = $derived(
		fitAnnotationBox(
			text,
			width,
			height,
			measure,
			border,
			textLayoutVersion === 1 ? 'strict' : 'legacy-height-only'
		)
	);
</script>

{#if border}<rect {x} {y} {width} {height} fill="none" stroke={color} stroke-width="0.75" />{/if}
<text
	x={x + layout.padding}
	font-size={layout.size}
	font-family="CorrespondenceSarabun"
	fill={color}
>
	{#each layout.lines as line, index (index)}<tspan
			x={x + layout.padding}
			y={y + layout.padding + layout.size * 1.1 + index * layout.size * 1.3}>{line}</tspan
		>{/each}
</text>
