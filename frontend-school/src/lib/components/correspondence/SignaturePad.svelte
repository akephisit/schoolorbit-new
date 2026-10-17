<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { pointerPoint, pathData, type InkPath } from '#lib/correspondence/geometry.js';
	let {
		paths = $bindable([]),
		disabled = false,
		onimage
	}: {
		paths?: InkPath[];
		disabled?: boolean;
		onimage?: (file: File) => void;
	} = $props();
	let surface: SVGSVGElement;
	let active = $state<number | null>(null);
	function start(event: PointerEvent) {
		if (disabled || active !== null || (event.pointerType === 'mouse' && event.button !== 0))
			return;
		event.preventDefault();
		surface.setPointerCapture(event.pointerId);
		active = event.pointerId;
		paths = [
			...paths,
			{ points: [pointerPoint(event.clientX, event.clientY, surface.getBoundingClientRect())] }
		];
	}
	function move(event: PointerEvent) {
		if (active !== event.pointerId) return;
		const point = pointerPoint(event.clientX, event.clientY, surface.getBoundingClientRect());
		paths = paths.map((path, index) =>
			index === paths.length - 1 ? { points: [...path.points, point] } : path
		);
	}
	function end(event: PointerEvent) {
		if (active !== event.pointerId) return;
		move(event);
		active = null;
		if (surface.hasPointerCapture(event.pointerId)) surface.releasePointerCapture(event.pointerId);
	}
</script>

<div class="space-y-3" data-testid="signature-pad">
	<p class="text-sm text-muted-foreground">
		วาดลายมือชื่อด้วยเมาส์ นิ้ว หรือปากกา หรือเลือกภาพลายมือชื่อ
	</p>
	<!-- A drawing surface is pointer-operated; keyboard users can use the labeled image upload. -->
	<svg
		bind:this={surface}
		viewBox="0 0 1000 1000"
		preserveAspectRatio="none"
		role="img"
		aria-label="พื้นที่วาดลายมือชื่อ"
		class="h-44 w-full touch-none rounded-md border bg-background text-foreground"
		onpointerdown={start}
		onpointermove={move}
		onpointerup={end}
		onpointercancel={() => {
			active = null;
		}}
	>
		{#each paths as path, index (index)}<path
				d={pathData(path.points)}
				fill="none"
				stroke="currentColor"
				stroke-width="6"
				stroke-linecap="round"
				stroke-linejoin="round"
			/>{/each}
	</svg>
	<div class="flex flex-wrap gap-2">
		<Button
			type="button"
			variant="outline"
			size="sm"
			disabled={disabled || !paths.length}
			onclick={() => {
				paths = paths.slice(0, -1);
			}}>ย้อนเส้นล่าสุด</Button
		>
		<Button
			type="button"
			variant="ghost"
			size="sm"
			disabled={disabled || !paths.length}
			onclick={() => {
				paths = [];
			}}>ล้างลายมือชื่อ</Button
		>
	</div>
	{#if onimage}
		<div class="space-y-2">
			<Label for="signature-image">ภาพลายมือชื่อส่วนตัว</Label><Input
				id="signature-image"
				type="file"
				accept="image/png,image/jpeg,image/webp"
				{disabled}
				onchange={(event) => {
					const file = event.currentTarget.files?.[0];
					if (file) onimage?.(file);
				}}
			/>
		</div>
	{/if}
</div>
