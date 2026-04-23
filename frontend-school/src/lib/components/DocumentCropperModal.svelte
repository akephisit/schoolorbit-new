<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import {
		Dialog,
		DialogContent,
		DialogHeader,
		DialogTitle,
		DialogFooter
	} from '$lib/components/ui/dialog';
	import { LoaderCircle } from 'lucide-svelte';

	// ===========================
	// Types
	// ===========================
	type Point = { x: number; y: number };

	interface Props {
		open: boolean;
		imageSrc: string | null;
		docLabel: string;
		initialCorners?: [Point, Point, Point, Point];
		onComplete: (blob: Blob, corners: [Point, Point, Point, Point]) => void;
		onCancel: () => void;
	}

	let {
		open = $bindable(false),
		imageSrc = null,
		docLabel,
		initialCorners,
		onComplete,
		onCancel
	}: Props = $props();

	// ===========================
	// State
	// ===========================
	let processing = $state(false);

	// Corners normalized 0-1 relative to natural image dimensions
	// Order: TL, TR, BR, BL
	let corners = $state<[Point, Point, Point, Point]>([
		{ x: 0.08, y: 0.08 },
		{ x: 0.92, y: 0.08 },
		{ x: 0.92, y: 0.92 },
		{ x: 0.08, y: 0.92 }
	]);
	let perspImgEl = $state<HTMLImageElement>();
	let perspContainer = $state<HTMLDivElement>();
	// Display geometry (letterbox)
	let dispOffsetX = $state(0);
	let dispOffsetY = $state(0);
	let dispScale = $state(1);
	let perspDraggingIdx = $state<number | null>(null);
	// Ready flag: true only after image has loaded and geometry computed
	let dispReady = $state(false);
	// Magnifier
	const MAG_SIZE = 128;
	const MAG_ZOOM = 4;
	let magnCanvasEl = $state<HTMLCanvasElement>();

	// ===========================
	// Reset corners on new image
	// ===========================
	$effect(() => {
		if (imageSrc) {
			dispReady = false;
			corners = initialCorners
				? (initialCorners.map((c) => ({ ...c })) as [Point, Point, Point, Point])
				: [
						{ x: 0.08, y: 0.08 },
						{ x: 0.92, y: 0.08 },
						{ x: 0.92, y: 0.92 },
						{ x: 0.08, y: 0.92 }
					];
		}
	});

	// ===========================
	// PERSPECTIVE MODE logic
	// ===========================
	function onPerspLoad() {
		if (!perspImgEl || !perspContainer) return;
		computeDispGeometry();
		dispReady = true;
	}

	function computeDispGeometry() {
		if (!perspImgEl || !perspContainer) return;
		const cw = perspContainer.clientWidth;
		const ch = perspContainer.clientHeight;
		const iw = perspImgEl.naturalWidth;
		const ih = perspImgEl.naturalHeight;
		dispScale = Math.min(cw / iw, ch / ih);
		dispOffsetX = (cw - iw * dispScale) / 2;
		dispOffsetY = (ch - ih * dispScale) / 2;
	}

	function cornerToDisplay(c: Point): Point {
		if (!perspImgEl) return { x: 0, y: 0 };
		return {
			x: dispOffsetX + c.x * perspImgEl.naturalWidth * dispScale,
			y: dispOffsetY + c.y * perspImgEl.naturalHeight * dispScale
		};
	}

	function displayToCorner(dx: number, dy: number): Point {
		if (!perspImgEl) return { x: 0, y: 0 };
		return {
			x: Math.max(0, Math.min(1, (dx - dispOffsetX) / (perspImgEl.naturalWidth * dispScale))),
			y: Math.max(0, Math.min(1, (dy - dispOffsetY) / (perspImgEl.naturalHeight * dispScale)))
		};
	}

	function getContainerCoords(e: MouseEvent | TouchEvent): Point {
		if (!perspContainer) return { x: 0, y: 0 };
		const rect = perspContainer.getBoundingClientRect();
		const clientX = 'touches' in e ? e.touches[0].clientX : (e as MouseEvent).clientX;
		const clientY = 'touches' in e ? e.touches[0].clientY : (e as MouseEvent).clientY;
		return { x: clientX - rect.left, y: clientY - rect.top };
	}

	function findNearestCorner(pt: Point): number | null {
		const THRESH = 32;
		let best = -1;
		let bestD = Infinity;
		corners.forEach((c, i) => {
			const dp = cornerToDisplay(c);
			const d = Math.hypot(dp.x - pt.x, dp.y - pt.y);
			if (d < THRESH && d < bestD) {
				bestD = d;
				best = i;
			}
		});
		return best >= 0 ? best : null;
	}

	function onPerspMouseDown(e: MouseEvent) {
		e.preventDefault();
		const pt = getContainerCoords(e);
		perspDraggingIdx = findNearestCorner(pt);
		if (perspDraggingIdx !== null) {
			window.addEventListener('mousemove', onPerspMouseMove);
			window.addEventListener('mouseup', onPerspMouseUp, { once: true });
		}
	}

	function onPerspMouseMove(e: MouseEvent) {
		if (perspDraggingIdx === null) return;
		const pt = getContainerCoords(e);
		corners[perspDraggingIdx] = displayToCorner(pt.x, pt.y);
	}

	function onPerspMouseUp() {
		perspDraggingIdx = null;
		window.removeEventListener('mousemove', onPerspMouseMove);
	}

	function onPerspTouchStart(e: TouchEvent) {
		if (e.touches.length !== 1) return;
		const pt = getContainerCoords(e);
		perspDraggingIdx = findNearestCorner(pt);
	}

	function onPerspTouchMove(e: TouchEvent) {
		if (perspDraggingIdx === null) return;
		if (e.cancelable) e.preventDefault();
		const pt = getContainerCoords(e);
		corners[perspDraggingIdx] = displayToCorner(pt.x, pt.y);
	}

	function onPerspTouchEnd() {
		perspDraggingIdx = null;
	}

	function polygonPoints(): string {
		return corners
			.map((c) => {
				const d = cornerToDisplay(c);
				return `${d.x},${d.y}`;
			})
			.join(' ');
	}

	// ===========================
	// Magnifier (loupe)
	// ===========================
	let magnPos = $derived.by(() => {
		if (perspDraggingIdx === null || !perspContainer || !dispReady) return null;
		const dp = cornerToDisplay(corners[perspDraggingIdx]);
		const cw = perspContainer.clientWidth;
		const ch = perspContainer.clientHeight;
		let x = Math.max(6, Math.min(cw - MAG_SIZE - 6, dp.x - MAG_SIZE / 2));
		let y = dp.y - MAG_SIZE - 28;
		if (y < 6) y = dp.y + 28;
		if (y + MAG_SIZE > ch - 6) y = Math.max(6, ch - MAG_SIZE - 6);
		return { x, y };
	});

	$effect(() => {
		if (perspDraggingIdx === null || !magnCanvasEl || !perspImgEl || !dispReady) return;
		const corner = corners[perspDraggingIdx]; // reactive dep
		const nx = corner.x * perspImgEl.naturalWidth;
		const ny = corner.y * perspImgEl.naturalHeight;
		const srcW = MAG_SIZE / MAG_ZOOM;
		const srcH = MAG_SIZE / MAG_ZOOM;
		const ctx = magnCanvasEl.getContext('2d')!;
		ctx.clearRect(0, 0, MAG_SIZE, MAG_SIZE);
		ctx.drawImage(perspImgEl, nx - srcW / 2, ny - srcH / 2, srcW, srcH, 0, 0, MAG_SIZE, MAG_SIZE);
		// Crosshair
		ctx.strokeStyle = 'rgba(59,130,246,0.9)';
		ctx.lineWidth = 1.5;
		ctx.beginPath();
		ctx.moveTo(MAG_SIZE / 2, 0);
		ctx.lineTo(MAG_SIZE / 2, MAG_SIZE);
		ctx.moveTo(0, MAG_SIZE / 2);
		ctx.lineTo(MAG_SIZE, MAG_SIZE / 2);
		ctx.stroke();
	});

	// ===========================
	// Math: Gaussian elimination
	// ===========================
	function gaussElim(A: number[][], b: number[]): number[] {
		const n = A.length;
		const M = A.map((row, i) => [...row, b[i]]);
		for (let c = 0; c < n; c++) {
			let maxR = c;
			for (let r = c + 1; r < n; r++) {
				if (Math.abs(M[r][c]) > Math.abs(M[maxR][c])) maxR = r;
			}
			[M[c], M[maxR]] = [M[maxR], M[c]];
			const p = M[c][c];
			if (Math.abs(p) < 1e-10) continue;
			for (let r = c + 1; r < n; r++) {
				const f = M[r][c] / p;
				for (let j = c; j <= n; j++) M[r][j] -= f * M[c][j];
			}
		}
		const x = new Array(n).fill(0);
		for (let i = n - 1; i >= 0; i--) {
			x[i] = M[i][n];
			for (let j = i + 1; j < n; j++) x[i] -= M[i][j] * x[j];
			if (Math.abs(M[i][i]) > 1e-10) x[i] /= M[i][i];
		}
		return x;
	}

	function computeHomography(src: Point[], dst: Point[]): number[] {
		const rows: number[][] = [];
		const rhs: number[] = [];
		for (let i = 0; i < 4; i++) {
			const { x: sx, y: sy } = src[i];
			const { x: dx, y: dy } = dst[i];
			rows.push([sx, sy, 1, 0, 0, 0, -dx * sx, -dx * sy]);
			rows.push([0, 0, 0, sx, sy, 1, -dy * sx, -dy * sy]);
			rhs.push(dx, dy);
		}
		const h = gaussElim(rows, rhs);
		return [...h, 1];
	}

	function applyH(H: number[], x: number, y: number): Point {
		const w = H[6] * x + H[7] * y + H[8];
		return { x: (H[0] * x + H[1] * y + H[2]) / w, y: (H[3] * x + H[4] * y + H[5]) / w };
	}

	function computeAffine(
		s0: Point,
		s1: Point,
		s2: Point,
		d0: Point,
		d1: Point,
		d2: Point
	): [number, number, number, number, number, number] {
		const M = [
			[s0.x, s0.y, 1],
			[s1.x, s1.y, 1],
			[s2.x, s2.y, 1]
		];
		const [a, c, e] = gaussElim(M, [d0.x, d1.x, d2.x]);
		const [b, d, f] = gaussElim(M, [d0.y, d1.y, d2.y]);
		return [a, b, c, d, e, f];
	}

	// ===========================
	// Final render
	// ===========================
	async function renderPerspective(): Promise<Blob> {
		if (!perspImgEl) throw new Error('No image');
		const iw = perspImgEl.naturalWidth;
		const ih = perspImgEl.naturalHeight;
		const src: Point[] = corners.map((c) => ({ x: c.x * iw, y: c.y * ih }));
		const topW = Math.hypot(src[1].x - src[0].x, src[1].y - src[0].y);
		const botW = Math.hypot(src[2].x - src[3].x, src[2].y - src[3].y);
		const lftH = Math.hypot(src[3].x - src[0].x, src[3].y - src[0].y);
		const rgtH = Math.hypot(src[2].x - src[1].x, src[2].y - src[1].y);
		let outW = Math.round((topW + botW) / 2);
		let outH = Math.round((lftH + rgtH) / 2);
		const maxDim = 2000;
		if (outW > maxDim || outH > maxDim) {
			const sc = maxDim / Math.max(outW, outH);
			outW = Math.round(outW * sc);
			outH = Math.round(outH * sc);
		}
		if (outW < 1) outW = 1;
		if (outH < 1) outH = 1;

		const dst: Point[] = [
			{ x: 0, y: 0 },
			{ x: outW, y: 0 },
			{ x: outW, y: outH },
			{ x: 0, y: outH }
		];

		const H_inv = computeHomography(dst, src);

		const outCanvas = document.createElement('canvas');
		outCanvas.width = outW;
		outCanvas.height = outH;
		const ctx = outCanvas.getContext('2d')!;

		const TILES = 32;
		for (let ty = 0; ty < TILES; ty++) {
			for (let tx = 0; tx < TILES; tx++) {
				const dx0 = (tx / TILES) * outW;
				const dy0 = (ty / TILES) * outH;
				const dx1 = ((tx + 1) / TILES) * outW;
				const dy1 = ((ty + 1) / TILES) * outH;

				const stl = applyH(H_inv, dx0, dy0);
				const str = applyH(H_inv, dx1, dy0);
				const sbl = applyH(H_inv, dx0, dy1);

				const dtl = { x: dx0, y: dy0 };
				const dtr = { x: dx1, y: dy0 };
				const dbl = { x: dx0, y: dy1 };
				const [a, b, c, d, e, f] = computeAffine(stl, str, sbl, dtl, dtr, dbl);

				ctx.save();
				ctx.beginPath();
				ctx.rect(dx0, dy0, dx1 - dx0 + 0.5, dy1 - dy0 + 0.5);
				ctx.clip();
				ctx.setTransform(a, b, c, d, e, f);
				ctx.drawImage(perspImgEl, 0, 0);
				ctx.restore();
			}
		}

		return new Promise((res) => outCanvas.toBlob((b) => res(b!), 'image/jpeg', 0.92));
	}

	async function handleConfirm() {
		processing = true;
		try {
			const blob = await renderPerspective();
			onComplete(blob, corners.map((c) => ({ ...c })) as [Point, Point, Point, Point]);
			open = false;
		} catch (err) {
			console.error('Crop failed:', err);
		} finally {
			processing = false;
		}
	}

	function handleCancel() {
		open = false;
		onCancel();
	}
</script>

<Dialog bind:open>
	<DialogContent class="w-full max-w-2xl p-0 gap-0 overflow-hidden rounded-xl">
		<DialogHeader class="px-4 pt-4 pb-3 border-b">
			<DialogTitle class="text-sm font-semibold truncate pr-6">{docLabel}</DialogTitle>
			<p class="text-xs text-muted-foreground mt-1">
				ลากจุด 4 มุมให้ครอบเอกสาร — ระบบจะปรับมุมเอียงให้ตรงอัตโนมัติ
			</p>
		</DialogHeader>

		{#if imageSrc}
			<div
				bind:this={perspContainer}
				class="relative w-full h-[340px] sm:h-[420px] bg-black/90 overflow-hidden touch-none select-none"
				onmousedown={onPerspMouseDown}
				ontouchstart={onPerspTouchStart}
				ontouchmove={onPerspTouchMove}
				ontouchend={onPerspTouchEnd}
				role="presentation"
			>
				<img
					bind:this={perspImgEl}
					src={imageSrc}
					alt=""
					class="absolute inset-0 w-full h-full object-contain pointer-events-none"
					onload={onPerspLoad}
					ondragstart={(e) => e.preventDefault()}
				/>

				{#if perspImgEl && dispReady}
					<svg class="absolute inset-0 w-full h-full overflow-visible" style="pointer-events: none">
						<polygon
							points={polygonPoints()}
							fill="rgba(59,130,246,0.10)"
							stroke="#3b82f6"
							stroke-width="2"
							stroke-dasharray="8,5"
						/>
						{#each corners as corner, i (i)}
							{@const dp = cornerToDisplay(corner)}
							<circle
								cx={dp.x}
								cy={dp.y}
								r="12"
								fill="white"
								stroke="#3b82f6"
								stroke-width="2.5"
								style="pointer-events: all; cursor: {perspDraggingIdx === i
									? 'grabbing'
									: 'grab'}; touch-action: none;"
							/>
							<circle cx={dp.x} cy={dp.y} r="3" fill="#3b82f6" style="pointer-events: none" />
						{/each}
					</svg>
					{#if magnPos !== null}
						<canvas
							bind:this={magnCanvasEl}
							width={MAG_SIZE}
							height={MAG_SIZE}
							class="absolute rounded-lg border-2 border-blue-500 shadow-xl pointer-events-none"
							style="left: {magnPos.x}px; top: {magnPos.y}px; z-index: 20; image-rendering: pixelated;"
						></canvas>
					{/if}
				{/if}
			</div>
		{/if}

		<DialogFooter class="px-4 py-3 border-t gap-2">
			<Button variant="outline" onclick={handleCancel} disabled={processing}>ยกเลิก</Button>
			<Button onclick={handleConfirm} disabled={processing || !imageSrc}>
				{#if processing}
					<LoaderCircle class="w-4 h-4 animate-spin mr-2" />
					กำลังประมวลผล...
				{:else}
					ยืนยัน
				{/if}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>
