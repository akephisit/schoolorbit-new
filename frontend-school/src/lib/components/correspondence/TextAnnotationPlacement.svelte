<script lang="ts">
	import type { DocumentAnnotation } from '#lib/api/correspondence.js';
	import {
		resizeWithinPage,
		pointerPoint,
		fromDisplay,
		type PageRotation
	} from '#lib/correspondence/geometry.js';
	let {
		annotation,
		rotation,
		bounds,
		selected,
		disabled,
		onselect,
		onchange
	}: {
		annotation: DocumentAnnotation;
		rotation: PageRotation;
		bounds: () => DOMRect;
		selected: boolean;
		disabled: boolean;
		onselect: () => void;
		onchange: (value: DocumentAnnotation) => void;
	} = $props();
	let drag: { pointerId: number; mode: 'move' | 'resize'; x: number; y: number } | null = null;
	function start(event: PointerEvent, mode: 'move' | 'resize') {
		if (disabled || (event.pointerType === 'mouse' && event.button !== 0)) return;
		event.preventDefault();
		event.stopPropagation();
		onselect();
		const point = pointerPoint(event.clientX, event.clientY, bounds(), rotation);
		drag = {
			pointerId: event.pointerId,
			mode,
			x: point.x - annotation.x,
			y: point.y - annotation.y
		};
		if (event.currentTarget instanceof HTMLElement)
			event.currentTarget.setPointerCapture(event.pointerId);
	}
	function move(event: PointerEvent) {
		if (disabled || drag?.pointerId !== event.pointerId) return;
		const point = pointerPoint(event.clientX, event.clientY, bounds(), rotation);
		onchange(
			drag.mode === 'resize'
				? resizeWithinPage(annotation, point.x - annotation.x, point.y - annotation.y)
				: {
						...annotation,
						x: Math.max(0, Math.min(1 - annotation.width, point.x - drag.x)),
						y: Math.max(0, Math.min(1 - annotation.height, point.y - drag.y))
					}
		);
	}
	function end(event: PointerEvent) {
		if (drag?.pointerId !== event.pointerId) return;
		drag = null;
		if (
			event.currentTarget instanceof HTMLElement &&
			event.currentTarget.hasPointerCapture(event.pointerId)
		)
			event.currentTarget.releasePointerCapture(event.pointerId);
	}
	function keyboard(event: KeyboardEvent, resize: boolean) {
		if (disabled || !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key))
			return;
		event.preventDefault();
		onselect();
		const delta = event.shiftKey ? 0.05 : 0.01;
		const point = fromDisplay(
			{
				x: 0.5 + (event.key === 'ArrowRight' ? delta : event.key === 'ArrowLeft' ? -delta : 0),
				y: 0.5 + (event.key === 'ArrowDown' ? delta : event.key === 'ArrowUp' ? -delta : 0)
			},
			rotation
		);
		onchange(
			resize
				? resizeWithinPage(
						annotation,
						annotation.width + point.x - 0.5,
						annotation.height + point.y - 0.5
					)
				: {
						...annotation,
						x: Math.max(0, Math.min(1 - annotation.width, annotation.x + point.x - 0.5)),
						y: Math.max(0, Math.min(1 - annotation.height, annotation.y + point.y - 0.5))
					}
		);
	}
</script>

<div
	class="absolute z-10"
	class:border-2={selected}
	class:border-dashed={selected}
	class:border-primary={selected}
	style:left={`${annotation.x * 100}%`}
	style:top={`${annotation.y * 100}%`}
	style:width={`${annotation.width * 100}%`}
	style:height={`${annotation.height * 100}%`}
	data-testid="text-annotation-placement"
>
	<button
		type="button"
		class="absolute inset-0 h-full w-full touch-none cursor-move focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
		aria-label="เลื่อนกรอบข้อความ ใช้ปุ่มลูกศรเพื่อปรับตำแหน่ง"
		{disabled}
		onclick={onselect}
		onfocus={onselect}
		onpointerdown={(event) => start(event, 'move')}
		onpointermove={move}
		onpointerup={end}
		onpointercancel={end}
		onkeydown={(event) => keyboard(event, false)}
	></button>
	{#if selected}<button
			type="button"
			class="absolute -right-3 -bottom-3 h-9 w-9 touch-none cursor-se-resize rounded-full border-2 border-primary bg-background text-primary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
			aria-label="ปรับขนาดกรอบข้อความ ใช้ปุ่มลูกศรเพื่อปรับขนาด"
			{disabled}
			onpointerdown={(event) => start(event, 'resize')}
			onpointermove={move}
			onpointerup={end}
			onpointercancel={end}
			onkeydown={(event) => keyboard(event, true)}>↔</button
		>{/if}
</div>
