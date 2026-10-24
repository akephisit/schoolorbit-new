<script lang="ts">
	import type { SignatureInput } from '#lib/api/correspondence.js';
	import {
		resizeWithinPage,
		pointerPoint,
		fromDisplay,
		pathData,
		type PageRotation,
		type InkPath
	} from '#lib/correspondence/geometry.js';
	import { authStore } from '#lib/stores/auth.js';
	import PrivateFileImage from '#lib/components/files/PrivateFileImage.svelte';
	let {
		placement,
		rotation,
		bounds,
		disabled,
		paths = [],
		fileId,
		localImage,
		onchange
	}: {
		placement: SignatureInput;
		rotation: PageRotation;
		bounds: () => DOMRect;
		disabled: boolean;
		paths?: InkPath[];
		fileId?: string;
		localImage?: Blob;
		onchange: (placement: SignatureInput) => void;
	} = $props();
	let localImageUrl = $state('');
	$effect(() => {
		if (!localImage) {
			localImageUrl = '';
			return;
		}
		const url = URL.createObjectURL(localImage);
		localImageUrl = url;
		return () => URL.revokeObjectURL(url);
	});
	let drag: { pointerId: number; mode: 'move' | 'resize'; x: number; y: number } | null = null;
	function start(event: PointerEvent, mode: 'move' | 'resize') {
		if (disabled || (event.pointerType === 'mouse' && event.button !== 0)) return;
		event.preventDefault();
		event.stopPropagation();
		const point = pointerPoint(event.clientX, event.clientY, bounds(), rotation);
		drag = { pointerId: event.pointerId, mode, x: point.x - placement.x, y: point.y - placement.y };
		if (event.currentTarget instanceof HTMLElement)
			event.currentTarget.setPointerCapture(event.pointerId);
	}
	function move(event: PointerEvent) {
		if (disabled || drag?.pointerId !== event.pointerId) return;
		const point = pointerPoint(event.clientX, event.clientY, bounds(), rotation);
		if (drag.mode === 'move')
			onchange({
				...placement,
				x: Math.max(0, Math.min(1 - placement.width, point.x - drag.x)),
				y: Math.max(0, Math.min(1 - placement.height, point.y - drag.y))
			});
		else onchange(resizeWithinPage(placement, point.x - placement.x, point.y - placement.y));
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
		const delta = event.shiftKey ? 0.05 : 0.01;
		const point = fromDisplay(
			{
				x: 0.5 + (event.key === 'ArrowRight' ? delta : event.key === 'ArrowLeft' ? -delta : 0),
				y: 0.5 + (event.key === 'ArrowDown' ? delta : event.key === 'ArrowUp' ? -delta : 0)
			},
			rotation
		);
		const dx = point.x - 0.5,
			dy = point.y - 0.5;
		onchange(
			resize
				? resizeWithinPage(placement, placement.width + dx, placement.height + dy)
				: {
						...placement,
						x: Math.max(0, Math.min(1 - placement.width, placement.x + dx)),
						y: Math.max(0, Math.min(1 - placement.height, placement.y + dy))
					}
		);
	}
</script>

<div
	class="absolute z-10 border-2 border-dashed border-primary bg-primary/5"
	style:left={`${placement.x * 100}%`}
	style:top={`${placement.y * 100}%`}
	style:width={`${placement.width * 100}%`}
	style:height={`${placement.height * 100}%`}
	data-testid="signature-placement"
>
	<button
		type="button"
		class="absolute inset-0 h-full w-full touch-none cursor-move focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
		aria-label="เลื่อนลายมือชื่อ ใช้ปุ่มลูกศรเพื่อปรับตำแหน่ง"
		{disabled}
		onpointerdown={(event) => start(event, 'move')}
		onpointermove={move}
		onpointerup={end}
		onpointercancel={end}
		onkeydown={(event) => keyboard(event, false)}
	>
		{#if localImageUrl}<img
				src={localImageUrl}
				alt="ตัวอย่างลายมือชื่อบนเอกสาร"
				class="h-full w-full object-contain"
			/>{:else if fileId}<PrivateFileImage
				{fileId}
				resourceId={$authStore.user?.id}
				alt="ตัวอย่างลายมือชื่อบนเอกสาร"
				class="h-full w-full object-contain"
			/>{:else if paths.length}<svg
				viewBox="0 0 1000 1000"
				preserveAspectRatio="none"
				class="h-full w-full text-foreground"
				aria-hidden="true"
				>{#each paths as path, index (index)}<path
						d={pathData(path.points)}
						fill="none"
						stroke="#111827"
						stroke-width="6"
						stroke-linecap="round"
					/>{/each}</svg
			>{:else}<span class="text-xs text-primary">ลายมือชื่อ</span>{/if}
	</button>
	<button
		type="button"
		class="absolute -right-3 -bottom-3 h-7 w-7 touch-none cursor-se-resize rounded-full border-2 border-primary bg-background text-xs text-primary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
		aria-label="ปรับขนาดลายมือชื่อ ใช้ปุ่มลูกศรเพื่อปรับขนาด"
		{disabled}
		onpointerdown={(event) => start(event, 'resize')}
		onpointermove={move}
		onpointerup={end}
		onpointercancel={end}
		onkeydown={(event) => keyboard(event, true)}>↔</button
	>
</div>
