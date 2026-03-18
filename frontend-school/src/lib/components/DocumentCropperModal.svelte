<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import {
		Dialog,
		DialogContent,
		DialogHeader,
		DialogTitle,
		DialogFooter
	} from '$lib/components/ui/dialog';
	import { LoaderCircle, Crop, ScanLine, ZoomIn, ZoomOut } from 'lucide-svelte';

	// ===========================
	// Types
	// ===========================
	type Point = { x: number; y: number };
	type Mode = 'rect' | 'perspective';

	interface Props {
		open: boolean;
		imageSrc: string | null;
		docLabel: string;
		onComplete: (blob: Blob) => void;
		onCancel: () => void;
	}

	let {
		open = $bindable(false),
		imageSrc = null,
		docLabel,
		onComplete,
		onCancel
	}: Props = $props();

	// ===========================
	// Shared state
	// ===========================
	let mode = $state<Mode>('rect');
	let processing = $state(false);

	// ===========================
	// RECT MODE state
	// ===========================
	let rectContainer = $state<HTMLDivElement>();
	let rectImg = $state<HTMLImageElement>();
	let zoom = $state(1);
	let baseScale = $state(1);
	let position = $state({ x: 0, y: 0 });
	let rectDragging = false;
	let rectDragStart = { x: 0, y: 0 };
	let naturalW = $state(0);
	let naturalH = $state(0);
	let maskW = $state(0);
	let maskH = $state(0);

	// ===========================
	// PERSPECTIVE MODE state
	// ===========================
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
	// Display geometry for perspective mode (letterbox)
	let dispOffsetX = $state(0);
	let dispOffsetY = $state(0);
	let dispScale = $state(1);
	let perspDraggingIdx = $state<number | null>(null);

	// ===========================
	// Reset on new image
	// ===========================
	$effect(() => {
		if (imageSrc) {
			mode = 'rect';
			zoom = 1;
			position = { x: 0, y: 0 };
			corners = [
				{ x: 0.08, y: 0.08 },
				{ x: 0.92, y: 0.08 },
				{ x: 0.92, y: 0.92 },
				{ x: 0.08, y: 0.92 }
			];
		}
	});

	// ===========================
	// RECT MODE logic
	// ===========================
	function onRectLoad() {
		if (!rectImg || !rectContainer) return;
		naturalW = rectImg.naturalWidth;
		naturalH = rectImg.naturalHeight;
		const cw = rectContainer.clientWidth;
		const ch = rectContainer.clientHeight;
		maskW = cw * 0.88;
		maskH = ch * 0.88;
		const sx = maskW / naturalW;
		const sy = maskH / naturalH;
		baseScale = Math.max(sx, sy);
		position = { x: 0, y: 0 };
		zoom = 1;
	}

	function rectLimits() {
		const s = baseScale * zoom;
		return {
			x: Math.max(0, (naturalW * s - maskW) / 2),
			y: Math.max(0, (naturalH * s - maskH) / 2)
		};
	}

	function onRectMouseDown(e: MouseEvent) {
		e.preventDefault();
		rectDragging = true;
		rectDragStart = { x: e.clientX - position.x, y: e.clientY - position.y };
		window.addEventListener('mousemove', onRectMouseMove);
		window.addEventListener('mouseup', onRectMouseUp, { once: true });
	}

	function onRectMouseMove(e: MouseEvent) {
		if (!rectDragging) return;
		const lim = rectLimits();
		position.x = Math.max(-lim.x, Math.min(e.clientX - rectDragStart.x, lim.x));
		position.y = Math.max(-lim.y, Math.min(e.clientY - rectDragStart.y, lim.y));
	}

	function onRectMouseUp() {
		rectDragging = false;
		window.removeEventListener('mousemove', onRectMouseMove);
	}

	function onRectTouchStart(e: TouchEvent) {
		if (e.touches.length !== 1) return;
		rectDragging = true;
		const t = e.touches[0];
		rectDragStart = { x: t.clientX - position.x, y: t.clientY - position.y };
		window.addEventListener('touchmove', onRectTouchMove, { passive: false });
		window.addEventListener('touchend', onRectTouchEnd, { once: true });
	}

	function onRectTouchMove(e: TouchEvent) {
		if (!rectDragging) return;
		if (e.cancelable) e.preventDefault();
		const t = e.touches[0];
		const lim = rectLimits();
		position.x = Math.max(-lim.x, Math.min(t.clientX - rectDragStart.x, lim.x));
		position.y = Math.max(-lim.y, Math.min(t.clientY - rectDragStart.y, lim.y));
	}

	function onRectTouchEnd() {
		rectDragging = false;
		window.removeEventListener('touchmove', onRectTouchMove);
	}

	function onWheel(e: WheelEvent) {
		e.preventDefault();
		zoom = Math.max(1, Math.min(5, zoom + (e.deltaY > 0 ? -0.12 : 0.12)));
	}

	// ===========================
	// PERSPECTIVE MODE logic
	// ===========================
	function onPerspLoad() {
		if (!perspImgEl || !perspContainer) return;
		computeDispGeometry();
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

	// Convert corner (normalized to natural) → SVG container coords
	function cornerToDisplay(c: Point): Point {
		if (!perspImgEl) return { x: 0, y: 0 };
		return {
			x: dispOffsetX + c.x * perspImgEl.naturalWidth * dispScale,
			y: dispOffsetY + c.y * perspImgEl.naturalHeight * dispScale
		};
	}

	// Convert container coords → normalized corner
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
		const THRESH = 32; // px
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

	// Display polygon points string for SVG
	function polygonPoints(): string {
		return corners.map((c) => {
			const d = cornerToDisplay(c);
			return `${d.x},${d.y}`;
		}).join(' ');
	}

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

	// Compute homography H (9 elements) mapping src[4] → dst[4]
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

	// Compute 2D affine [a,b,c,d,e,f] for ctx.transform
	// maps: s0→d0, s1→d1, s2→d2
	function computeAffine(
		s0: Point, s1: Point, s2: Point,
		d0: Point, d1: Point, d2: Point
	): [number, number, number, number, number, number] {
		const M = [
			[s0.x, s0.y, 1],
			[s1.x, s1.y, 1],
			[s2.x, s2.y, 1]
		];
		const [a, c, e] = gaussElim(M, [d0.x, d1.x, d2.x]);
		const [b, d, f] = gaussElim(M, [d0.y, d1.y, d2.y]);
		// ctx.transform(a, b, c, d, e, f): new_x = a*x + c*y + e, new_y = b*x + d*y + f
		return [a, b, c, d, e, f];
	}

	// ===========================
	// Sharpen helper
	// ===========================
	function sharpenCanvas(canvas: HTMLCanvasElement): void {
		const ctx = canvas.getContext('2d')!;
		const w = canvas.width;
		const h = canvas.height;
		const src = ctx.getImageData(0, 0, w, h);
		const dst = ctx.createImageData(w, h);
		const s = src.data;
		const d = dst.data;
		// Kernel: [0,-1,0,-1,5,-1,0,-1,0]
		for (let y = 1; y < h - 1; y++) {
			for (let x = 1; x < w - 1; x++) {
				for (let c = 0; c < 3; c++) {
					const i = (y * w + x) * 4 + c;
					d[i] = Math.max(
						0,
						Math.min(
							255,
							-s[((y - 1) * w + x) * 4 + c] -
								s[(y * w + x - 1) * 4 + c] +
								5 * s[i] -
								s[(y * w + x + 1) * 4 + c] -
								s[((y + 1) * w + x) * 4 + c]
						)
					);
				}
				d[(y * w + x) * 4 + 3] = s[(y * w + x) * 4 + 3];
			}
		}
		ctx.putImageData(dst, 0, 0);
	}

	// ===========================
	// Final render
	// ===========================
	async function renderRect(): Promise<Blob> {
		if (!rectImg) throw new Error('No image');
		const s = baseScale * zoom;
		const natCX = naturalW / 2 - position.x / s;
		const natCY = naturalH / 2 - position.y / s;
		const natMW = maskW / s;
		const natMH = maskH / s;
		const sx = Math.max(0, natCX - natMW / 2);
		const sy = Math.max(0, natCY - natMH / 2);
		const sw = Math.min(natMW, naturalW - sx);
		const sh = Math.min(natMH, naturalH - sy);
		let outW = Math.round(sw);
		let outH = Math.round(sh);
		const maxDim = 2000;
		if (outW > maxDim || outH > maxDim) {
			const sc = maxDim / Math.max(outW, outH);
			outW = Math.round(outW * sc);
			outH = Math.round(outH * sc);
		}
		const canvas = document.createElement('canvas');
		canvas.width = outW;
		canvas.height = outH;
		const ctx = canvas.getContext('2d')!;
		ctx.fillStyle = '#fff';
		ctx.fillRect(0, 0, outW, outH);
		ctx.drawImage(rectImg, sx, sy, sw, sh, 0, 0, outW, outH);
		sharpenCanvas(canvas);
		return new Promise((res) => canvas.toBlob((b) => res(b!), 'image/jpeg', 0.92));
	}

	async function renderPerspective(): Promise<Blob> {
		if (!perspImgEl) throw new Error('No image');
		const iw = perspImgEl.naturalWidth;
		const ih = perspImgEl.naturalHeight;
		// Source corners in natural pixel space
		const src: Point[] = corners.map((c) => ({ x: c.x * iw, y: c.y * ih }));
		// Estimate output dimensions from corner distances
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

		// Inverse homography: output pixel → source pixel
		const H_inv = computeHomography(dst, src);

		const outCanvas = document.createElement('canvas');
		outCanvas.width = outW;
		outCanvas.height = outH;
		const ctx = outCanvas.getContext('2d')!;

		// Tile-based rendering: approximate perspective per tile with affine
		const TILES = 32;
		for (let ty = 0; ty < TILES; ty++) {
			for (let tx = 0; tx < TILES; tx++) {
				const dx0 = (tx / TILES) * outW;
				const dy0 = (ty / TILES) * outH;
				const dx1 = ((tx + 1) / TILES) * outW;
				const dy1 = ((ty + 1) / TILES) * outH;

				// Source corners via H_inv
				const stl = applyH(H_inv, dx0, dy0);
				const str = applyH(H_inv, dx1, dy0);
				const sbl = applyH(H_inv, dx0, dy1);

				// Forward affine: src → dst
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

		return new Promise((res) => outCanvas.toBlob((b) => res(b!), 'image/jpeg', 0.88));
	}

	async function handleConfirm() {
		processing = true;
		try {
			const blob = mode === 'rect' ? await renderRect() : await renderPerspective();
			onComplete(blob);
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
			<!-- Mode tabs -->
			<div class="flex gap-1 mt-2">
				<button
					type="button"
					class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors
						{mode === 'rect'
						? 'bg-blue-100 text-blue-700'
						: 'text-muted-foreground hover:bg-muted'}"
					onclick={() => (mode = 'rect')}
				>
					<Crop class="w-3.5 h-3.5" />
					ครอปตรง
				</button>
				<button
					type="button"
					class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors
						{mode === 'perspective'
						? 'bg-blue-100 text-blue-700'
						: 'text-muted-foreground hover:bg-muted'}"
					onclick={() => (mode = 'perspective')}
				>
					<ScanLine class="w-3.5 h-3.5" />
					ปรับมุมเอียง
				</button>
			</div>
		</DialogHeader>

		{#if imageSrc}
			<!-- ===== RECT MODE ===== -->
			{#if mode === 'rect'}
				<div
					bind:this={rectContainer}
					class="relative w-full h-[340px] sm:h-[380px] bg-black/90 overflow-hidden touch-none select-none cursor-move"
					onmousedown={onRectMouseDown}
					ontouchstart={onRectTouchStart}
					ontouchmove={onRectTouchMove}
					ontouchend={onRectTouchEnd}
					onwheel={onWheel}
					role="presentation"
				>
					<img
						bind:this={rectImg}
						src={imageSrc}
						alt="crop target"
						class="absolute max-w-none pointer-events-none origin-center"
						style:left="50%"
						style:top="50%"
						style:transform="translate(calc(-50% + {position.x}px), calc(-50% + {position.y}px)) scale({baseScale * zoom})"
						onload={onRectLoad}
						ondragstart={(e) => e.preventDefault()}
					/>
					<!-- Mask overlay (cut-out rect) -->
					{#if maskW && maskH}
						<div class="absolute inset-0 pointer-events-none flex items-center justify-center">
							<div
								class="absolute border-2 border-white/70 shadow-[0_0_0_9999px_rgba(0,0,0,0.62)] z-10"
								style:width="{maskW}px"
								style:height="{maskH}px"
							></div>
							<!-- Corner brackets -->
							{#each [[-1, -1], [1, -1], [1, 1], [-1, 1]] as [sx, sy], i}
								<div
									class="absolute z-20 w-5 h-5 pointer-events-none"
									style:left="{50 + (sx * maskW) / 2}px"
									style:top="{50 + (sy * maskH) / 2}px"
									style:transform="translate({sx < 0 ? '-100%' : '0'}, {sy < 0 ? '-100%' : '0'})"
								>
									<div
										class="absolute border-white border-t-2 border-l-2 w-4 h-4"
										style:transform="rotate({i * 90}deg)"
									></div>
								</div>
							{/each}
						</div>
					{/if}
				</div>

				<!-- Zoom controls -->
				<div class="flex items-center gap-3 px-4 py-2 border-t bg-muted/30">
					<ZoomOut class="w-4 h-4 text-muted-foreground shrink-0" />
					<input
						type="range"
						min="1"
						max="5"
						step="0.05"
						bind:value={zoom}
						class="w-full h-1.5 rounded-full accent-blue-500"
					/>
					<ZoomIn class="w-4 h-4 text-muted-foreground shrink-0" />
					<span class="text-xs text-muted-foreground w-8 shrink-0">{zoom.toFixed(1)}x</span>
				</div>

			<!-- ===== PERSPECTIVE MODE ===== -->
			{:else}
				<div
					bind:this={perspContainer}
					class="relative w-full h-[340px] sm:h-[380px] bg-black/90 overflow-hidden touch-none select-none"
					onmousedown={onPerspMouseDown}
					ontouchstart={onPerspTouchStart}
					ontouchmove={onPerspTouchMove}
					ontouchend={onPerspTouchEnd}
					role="presentation"
				>
					<!-- Hidden img for natural dimensions -->
					<img
						bind:this={perspImgEl}
						src={imageSrc}
						alt=""
						class="absolute inset-0 w-full h-full object-contain pointer-events-none"
						onload={onPerspLoad}
						ondragstart={(e) => e.preventDefault()}
					/>

					<!-- SVG overlay for corner handles -->
					{#if perspImgEl}
						<svg class="absolute inset-0 w-full h-full overflow-visible" style="pointer-events: none">
							<!-- Quad fill + stroke -->
							<polygon
								points={polygonPoints()}
								fill="rgba(59,130,246,0.10)"
								stroke="#3b82f6"
								stroke-width="2"
								stroke-dasharray="8,5"
							/>
							<!-- Corner handles -->
							{#each corners as corner, i}
								{@const dp = cornerToDisplay(corner)}
								<circle
									cx={dp.x}
									cy={dp.y}
									r="12"
									fill="white"
									stroke="#3b82f6"
									stroke-width="2.5"
									style="pointer-events: all; cursor: {perspDraggingIdx === i ? 'grabbing' : 'grab'}; touch-action: none;"
								/>
								<circle
									cx={dp.x}
									cy={dp.y}
									r="3"
									fill="#3b82f6"
									style="pointer-events: none"
								/>
							{/each}
						</svg>
					{/if}
				</div>
				<p class="text-xs text-center text-muted-foreground px-4 py-2 border-t bg-muted/30">
					ลากจุด 4 มุมให้ครอบเอกสาร — ระบบจะปรับมุมเอียงให้ตรงอัตโนมัติ
				</p>
			{/if}
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
