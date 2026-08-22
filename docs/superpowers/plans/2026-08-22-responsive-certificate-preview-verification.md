# Responsive Certificate Preview and Public Verification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the editor, staff issuance review, and public verification use one responsive read-only certificate preview system, with automatic issued-certificate rendering after manual or QR verification.

**Architecture:** A pure fit helper calculates logical canvas size and bounded render resolution from the preview stage's actual content box. Shared Svelte components own read-only canvas rendering, loading/error states, cancellation, retry, and fullscreen presentation; each workflow continues to own manifest authorization and acquisition. The interactive editor canvas stays separate, while its real-PDF dialog adopts the shared read-only viewer.

**Tech Stack:** SvelteKit 5 runes, TypeScript, Tailwind CSS, local shadcn-svelte dialog/button primitives, `ResizeObserver`, the existing lazy certificate renderer, Node test runner, and Playwright.

**Spec:** `docs/superpowers/specs/2026-08-22-responsive-certificate-preview-verification-design.md`

## Global Constraints

- This is frontend-only: do not modify the backend, migrations, permissions, OpenAPI, or generated TypeScript contracts.
- Use one shared read-only preview surface for editor real-PDF preview, staff candidate preview, and public issued preview.
- Keep `CertificateCanvas.svelte` as the editor's interactive drag/resize/rotate surface.
- Automatically request and render a preview only when public verification returns `status === 'issued'` with a non-empty receipt.
- A revoked or invalid certificate must never request the public render manifest, load the renderer, show the old certificate, or expose PDF download.
- Fit the entire page without cropping, stretching, or horizontal preview scrolling; preserve page aspect ratio.
- The standard staff/editor dialog is constrained to `96vw` by `94dvh`; fullscreen is the only enlargement control. Do not add zoom or pan.
- Use exact loading copy: `กำลังโหลดฟอนต์และสร้างตัวอย่าง…` for staff/editor and `กำลังสร้างภาพเกียรติบัตร…` for public verification.
- Keep QR proof only in component memory after scrubbing it from the URL fragment. Never store or log proof, receipt, signed URL, object key, or raw public error details.
- Abort stale manifest/render work when the source identity changes, the dialog closes, navigation occurs, or the component unmounts.
- Reuse the existing lazy renderer; add no frontend dependency.
- Write Svelte 5 runes code and run the Svelte autofixer on every changed `.svelte` file.
- Run every test and verification command sequentially with Playwright `--workers=1`.

---

## File Map

### New files

- `frontend-school/src/lib/certificates/preview-fit.ts` — pure fit and bounded pixel-resolution calculation plus the shared preview-state type.
- `frontend-school/src/lib/components/certificates/CertificatePreviewSurface.svelte` — measured canvas surface, renderer lifecycle, spinner, safe error, retry, and state callback.
- `frontend-school/src/lib/components/certificates/CertificatePreviewFullscreenDialog.svelte` — accessible fullscreen dialog that embeds the shared surface.
- `frontend-school/src/lib/components/certificates/CertificatePreviewDialog.svelte` — standard `96vw` × `94dvh` staff/editor wrapper plus fullscreen action.
- `frontend-school/tests/static/certificate-preview.test.mjs` — executable tests for fit math and source-level shared-component boundaries.

### Modified files

- `frontend-school/src/lib/components/certificates/editor/CertificateEditor.svelte` — replace its duplicate canvas/dialog renderer with parent-owned manifest acquisition and `CertificatePreviewDialog`.
- `frontend-school/src/lib/components/certificates/CertificateIssueRequestReview.svelte` — replace its window-sized candidate canvas with `CertificatePreviewDialog`.
- `frontend-school/src/lib/components/certificates/PublicCertificateVerification.svelte` — retain ephemeral verification context, auto-load an issued manifest, embed `CertificatePreviewSurface`, open the shared fullscreen dialog, and reshape successful results.
- `frontend-school/tests/static/certificate-editor.test.mjs` — assert the editor's real-PDF preview uses the shared dialog and retains unsaved-layout payloads.
- `frontend-school/tests/static/certificate-request-ui.test.mjs` — assert candidate preview uses the shared dialog and no window-based scale.
- `frontend-school/tests/static/certificate-public-verification.test.mjs` — assert automatic issued preview and the revoked gating boundary.
- `frontend-school/tests/e2e/certificate-editor.spec.ts` — exercise shared loading, fit, retry, fullscreen, and `Escape` from the editor.
- `frontend-school/tests/e2e/certificate-request-workflow.spec.ts` — reproduce and prevent the wide-viewport staff-dialog overflow.
- `frontend-school/tests/e2e/certificate-public-verification.spec.ts` — exercise automatic manual/QR preview, retry, stale cancellation, independent download, revoked gating, responsive order, and fullscreen.

---

### Task 1: Pure Responsive Preview Fit Contract

**Files:**
- Create: `frontend-school/src/lib/certificates/preview-fit.ts`
- Create: `frontend-school/tests/static/certificate-preview.test.mjs`

**Interfaces:**
- Consumes: page width/height in displayed PDF points, measured stage width/height in CSS pixels, and `window.devicePixelRatio`.
- Produces: `calculateCertificatePreviewFit(input: CertificatePreviewFitInput): CertificatePreviewFit | null` and `CertificatePreviewState = 'idle' | 'loading' | 'ready' | 'error'`.

- [ ] **Step 1: Write failing fit tests**

Create `frontend-school/tests/static/certificate-preview.test.mjs` with executable portrait, landscape, invalid-size, and DPR-cap cases:

```js
import assert from 'node:assert/strict';
import test from 'node:test';

test('preview fit uses the limiting dimension and preserves landscape ratio', async () => {
	const { calculateCertificatePreviewFit } = await import(
		'../../src/lib/certificates/preview-fit.ts'
	);
	const fit = calculateCertificatePreviewFit({
		availableWidth: 960,
		availableHeight: 540,
		pageWidthPoints: 842,
		pageHeightPoints: 595,
		devicePixelRatio: 2
	});
	assert.ok(fit);
	assert.ok(Math.abs(fit.cssHeight - 540) < 1e-9);
	assert.ok(Math.abs(fit.cssWidth / fit.cssHeight - 842 / 595) < 1e-9);
	assert.ok(fit.renderScale <= 2);
});

test('preview fit uses width for portrait paper and rejects an unmeasured stage', async () => {
	const { calculateCertificatePreviewFit } = await import(
		'../../src/lib/certificates/preview-fit.ts'
	);
	const portrait = calculateCertificatePreviewFit({
		availableWidth: 360,
		availableHeight: 700,
		pageWidthPoints: 595,
		pageHeightPoints: 842,
		devicePixelRatio: 1
	});
	assert.ok(portrait);
	assert.ok(Math.abs(portrait.cssWidth - 360) < 1e-9);
	assert.equal(
		calculateCertificatePreviewFit({
			availableWidth: 0,
			availableHeight: 700,
			pageWidthPoints: 595,
			pageHeightPoints: 842,
			devicePixelRatio: 1
		}),
		null
	);
});

test('preview fit caps high-DPI rendering without changing logical size', async () => {
	const { calculateCertificatePreviewFit } = await import(
		'../../src/lib/certificates/preview-fit.ts'
	);
	const fit = calculateCertificatePreviewFit({
		availableWidth: 842,
		availableHeight: 595,
		pageWidthPoints: 842,
		pageHeightPoints: 595,
		devicePixelRatio: 4
	});
	assert.deepEqual(fit, {
		logicalScale: 1,
		cssWidth: 842,
		cssHeight: 595,
		renderScale: 2
	});
});

```

- [ ] **Step 2: Run the math tests and verify they fail because the helper is absent**

Run:

```bash
cd frontend-school
node --test tests/static/certificate-preview.test.mjs
```

Expected: FAIL with `ERR_MODULE_NOT_FOUND` for `preview-fit.ts`.

- [ ] **Step 3: Implement the pure fit helper**

Create `frontend-school/src/lib/certificates/preview-fit.ts`:

```ts
export const MAX_CERTIFICATE_PREVIEW_DPR = 2;
export const MAX_CERTIFICATE_PREVIEW_RENDER_SCALE = 2;

export type CertificatePreviewState = 'idle' | 'loading' | 'ready' | 'error';

export type CertificatePreviewFitInput = {
	availableWidth: number;
	availableHeight: number;
	pageWidthPoints: number;
	pageHeightPoints: number;
	devicePixelRatio: number;
};

export type CertificatePreviewFit = {
	logicalScale: number;
	cssWidth: number;
	cssHeight: number;
	renderScale: number;
};

function finitePositive(value: number): boolean {
	return Number.isFinite(value) && value > 0;
}

export function calculateCertificatePreviewFit(
	input: CertificatePreviewFitInput
): CertificatePreviewFit | null {
	const values = [
		input.availableWidth,
		input.availableHeight,
		input.pageWidthPoints,
		input.pageHeightPoints
	];
	if (!values.every(finitePositive)) return null;

	const logicalScale = Math.min(
		input.availableWidth / input.pageWidthPoints,
		input.availableHeight / input.pageHeightPoints
	);
	const deviceScale = Math.min(
		MAX_CERTIFICATE_PREVIEW_DPR,
		Math.max(1, finitePositive(input.devicePixelRatio) ? input.devicePixelRatio : 1)
	);

	return {
		logicalScale,
		cssWidth: input.pageWidthPoints * logicalScale,
		cssHeight: input.pageHeightPoints * logicalScale,
		renderScale: Math.min(
			MAX_CERTIFICATE_PREVIEW_RENDER_SCALE,
			logicalScale * deviceScale
		)
	};
}
```

- [ ] **Step 4: Run the focused math tests and verify they pass**

Run:

```bash
cd frontend-school
node --test tests/static/certificate-preview.test.mjs
```

Expected: all 3 fit tests PASS.

- [ ] **Step 5: Commit the fit contract**

```bash
git add frontend-school/src/lib/certificates/preview-fit.ts frontend-school/tests/static/certificate-preview.test.mjs
git commit -m "feat(certificates): add responsive preview fit contract"
```

---

### Task 2: Shared Preview Components and Editor Real-PDF Integration

**Files:**
- Create: `frontend-school/src/lib/components/certificates/CertificatePreviewSurface.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificatePreviewFullscreenDialog.svelte`
- Create: `frontend-school/src/lib/components/certificates/CertificatePreviewDialog.svelte`
- Modify: `frontend-school/src/lib/components/certificates/editor/CertificateEditor.svelte`
- Modify: `frontend-school/tests/static/certificate-preview.test.mjs`
- Modify: `frontend-school/tests/static/certificate-editor.test.mjs`
- Modify: `frontend-school/tests/e2e/certificate-editor.spec.ts`

**Interfaces:**
- Consumes: `calculateCertificatePreviewFit` and `CertificatePreviewState` from Task 1; a typed `CertificateRenderManifest | null`; parent-owned `manifestLoading` and `manifestError`.
- Produces: shared component props `manifest`, `manifestLoading`, `manifestError`, `ariaLabel`, `loadingLabel`, `renderFailureMessage`, `retryLabel`, `onretry`, and optional `onstatechange`; `CertificatePreviewDialog` additionally consumes `open`, `title`, `description`, and `onopenchange`.

- [ ] **Step 1: Extend editor tests for the shared boundary, true container fit, and fullscreen**

Extend `certificate-preview.test.mjs` with the shared source-boundary test, adding `readFile` from `node:fs/promises` and `const projectRoot = new URL('../../', import.meta.url)` at file scope:

```js
test('shared preview source files have focused ownership', async () => {
	const [surface, dialog, fullscreen] = await Promise.all([
		readFile(
			new URL('src/lib/components/certificates/CertificatePreviewSurface.svelte', projectRoot),
			'utf8'
		),
		readFile(
			new URL('src/lib/components/certificates/CertificatePreviewDialog.svelte', projectRoot),
			'utf8'
		),
		readFile(
			new URL(
				'src/lib/components/certificates/CertificatePreviewFullscreenDialog.svelte',
				projectRoot
			),
			'utf8'
		)
	]);
	assert.match(surface, /calculateCertificatePreviewFit/);
	assert.match(surface, /ResizeObserver/);
	assert.match(surface, /loadCertificateRenderer/);
	assert.match(dialog, /CertificatePreviewSurface/);
	assert.match(dialog, /CertificatePreviewFullscreenDialog/);
	assert.match(fullscreen, /CertificatePreviewSurface/);
});
```

Add these static assertions to `certificate-editor.test.mjs`:

```js
test('editor real PDF preview delegates rendering UI to the shared preview dialog', async () => {
	const source = await readFile(
		new URL('src/lib/components/certificates/editor/CertificateEditor.svelte', projectRoot),
		'utf8'
	);
	assert.match(source, /CertificatePreviewDialog/);
	assert.doesNotMatch(source, /window\.innerWidth[\s\S]*freshManifest\.pageGeometry/);
	assert.doesNotMatch(source, /<canvas[\s\S]*ผลพรีวิว PDF จริง/);
});
```

In `certificate-editor.spec.ts`, update the expected loading copy to `กำลังโหลดฟอนต์และสร้างตัวอย่าง…` and the simulated renderer-error assertion to the safe UI copy `สร้างพรีวิว PDF จริงไม่สำเร็จ`, then add fit/fullscreen assertions after a successful preview:

```ts
const previewStage = page.getByTestId('certificate-preview-stage').first();
await expect(previewStage).toBeVisible();
const fit = await previewStage.evaluate((stage) => {
	const canvas = stage.querySelector('canvas');
	if (!(canvas instanceof HTMLCanvasElement)) throw new Error('preview canvas missing');
	const stageRect = stage.getBoundingClientRect();
	const canvasRect = canvas.getBoundingClientRect();
	return {
		horizontalOverflow: stage.scrollWidth > stage.clientWidth + 1,
		fitsWidth: canvasRect.left >= stageRect.left - 1 && canvasRect.right <= stageRect.right + 1,
		fitsHeight: canvasRect.top >= stageRect.top - 1 && canvasRect.bottom <= stageRect.bottom + 1
	};
});
expect(fit).toEqual({ horizontalOverflow: false, fitsWidth: true, fitsHeight: true });

await page.getByRole('button', { name: 'ขยายเต็มจอ' }).click();
const fullscreen = page.getByRole('dialog', { name: 'พรีวิว PDF จริงแบบเต็มจอ' });
await expect(fullscreen).toBeVisible();
await page.keyboard.press('Escape');
await expect(fullscreen).toBeHidden();
await expect(previewDialog).toBeVisible();
```

- [ ] **Step 2: Run the new static architecture test and verify it fails**

Run the new source-boundary test:

```bash
cd frontend-school
node --test --test-name-pattern='shared preview source files' tests/static/certificate-preview.test.mjs
```

Expected: FAIL because the three shared components do not exist yet. The editor-specific architecture assertion also remains red until Step 5.

- [ ] **Step 3: Build `CertificatePreviewSurface.svelte`**

Implement typed props and renderer lifecycle with these core declarations and behavior:

```svelte
<script lang="ts">
	import type { CertificateRenderManifest } from '$lib/api/certificates';
	import {
		calculateCertificatePreviewFit,
		type CertificatePreviewState
	} from '$lib/certificates/preview-fit';
	import { loadCertificateRenderer } from '$lib/certificates/renderer';
	import { Button } from '$lib/components/ui/button';
	import { AlertTriangle, LoaderCircle, RefreshCw } from 'lucide-svelte';

	let {
		manifest,
		manifestLoading = false,
		manifestError = '',
		ariaLabel,
		loadingLabel,
		renderFailureMessage,
		retryLabel = 'ลองใหม่',
		onretry,
		onstatechange
	}: {
		manifest: CertificateRenderManifest | null;
		manifestLoading?: boolean;
		manifestError?: string;
		ariaLabel: string;
		loadingLabel: string;
		renderFailureMessage: string;
		retryLabel?: string;
		onretry: () => void;
		onstatechange?: (state: CertificatePreviewState) => void;
	} = $props();

	let canvas = $state<HTMLCanvasElement>();
	let viewport = $state.raw({ width: 0, height: 0 });
	let renderState = $state<CertificatePreviewState>('idle');
	let renderError = $state('');

	const fit = $derived(
		manifest
			? calculateCertificatePreviewFit({
					availableWidth: viewport.width,
					availableHeight: viewport.height,
					pageWidthPoints: manifest.pageGeometry.displayedWidthPoints,
					pageHeightPoints: manifest.pageGeometry.displayedHeightPoints,
					devicePixelRatio: typeof window === 'undefined' ? 1 : window.devicePixelRatio
				})
			: null
	);
	const state = $derived<CertificatePreviewState>(
		manifestError || renderError
			? 'error'
			: manifestLoading || (manifest && renderState !== 'ready')
				? 'loading'
				: manifest
					? 'ready'
					: 'idle'
	);

	function observeViewport(node: HTMLElement) {
		const observer = new ResizeObserver(([entry]) => {
			if (!entry) return;
			viewport = { width: entry.contentRect.width, height: entry.contentRect.height };
		});
		observer.observe(node);
		return () => observer.disconnect();
	}

	function captureCanvas(node: HTMLCanvasElement) {
		canvas = node;
		return () => {
			if (canvas === node) canvas = undefined;
		};
	}

	$effect(() => onstatechange?.(state));

	$effect(() => {
		const currentManifest = manifest;
		const currentFit = fit;
		const currentCanvas = canvas;
		if (manifestLoading || manifestError || !currentManifest || !currentFit || !currentCanvas) {
			renderState = 'idle';
			return;
		}
		const controller = new AbortController();
		renderState = 'loading';
		renderError = '';
		currentCanvas.width = 1;
		currentCanvas.height = 1;
		void (async () => {
			const renderer = await loadCertificateRenderer();
			await renderer.renderPreview(currentManifest, currentCanvas, {
				scale: currentFit.renderScale,
				signal: controller.signal
			});
			controller.signal.throwIfAborted();
			currentCanvas.style.width = `${currentFit.cssWidth}px`;
			currentCanvas.style.height = `${currentFit.cssHeight}px`;
			renderState = 'ready';
		})().catch(() => {
			if (controller.signal.aborted) return;
			renderError = renderFailureMessage;
			renderState = 'error';
		});
		return () => controller.abort();
	});
</script>

<div
	{@attach observeViewport}
	class="relative grid size-full min-h-72 place-items-center overflow-hidden rounded-lg bg-slate-200 p-4"
	data-testid="certificate-preview-stage"
	aria-busy={state === 'loading'}
>
	{#if state === 'loading'}
		<div class="grid place-items-center gap-3 text-center text-sm text-muted-foreground" role="status" aria-live="polite">
			<LoaderCircle class="size-7 animate-spin text-primary motion-reduce:animate-none" aria-hidden="true" />
			<p>{loadingLabel}</p>
		</div>
	{:else if state === 'error'}
		<div class="grid max-w-md place-items-center gap-3 text-center text-sm text-destructive" role="alert">
			<AlertTriangle class="size-6" aria-hidden="true" />
			<p>{manifestError || renderError}</p>
			<Button variant="secondary" onclick={onretry}>
				<RefreshCw class="size-4" aria-hidden="true" /> {retryLabel}
			</Button>
		</div>
	{/if}
	<canvas
		{@attach captureCanvas}
		hidden={state !== 'ready'}
		class="block max-h-full max-w-full bg-white shadow-xl"
		aria-label={ariaLabel}
	></canvas>
</div>
```

The canvas must never render the raw caught error. If the shared stage's Tailwind sizing needs a local wrapper to satisfy `min-height: 0` in both dialog and inline contexts, keep the same test ID on the measured content box rather than measuring the browser window.

- [ ] **Step 4: Build the standard and fullscreen dialog wrappers**

Both wrappers declare the same typed surface props, including `retryLabel?: string`, and forward them unchanged. `CertificatePreviewFullscreenDialog.svelte` uses the local dialog primitive and the surface:

```svelte
<Dialog.Root {open} onOpenChange={onopenchange}>
	<Dialog.Content class="flex h-dvh w-screen max-w-none flex-col overflow-hidden rounded-none border-0 p-3">
		<Dialog.Header class="shrink-0 px-2 pt-1">
			<Dialog.Title>{title}แบบเต็มจอ</Dialog.Title>
		</Dialog.Header>
		<div class="min-h-0 flex-1">
			<CertificatePreviewSurface
				{manifest}
				{manifestLoading}
				{manifestError}
				{ariaLabel}
				{loadingLabel}
				{renderFailureMessage}
				{retryLabel}
				{onretry}
			/>
		</div>
	</Dialog.Content>
</Dialog.Root>
```

`CertificatePreviewDialog.svelte` uses a normal dialog plus a local `fullscreenOpen` state, renders the shared surface in `min-h-0 flex-1`, and includes `ขยายเต็มจอ` and `ปิด` buttons. Use these sizing classes on normal content:

```svelte
class="flex h-[94dvh] w-[96vw] max-w-[96vw] flex-col overflow-hidden p-3"
```

Use this wrapper structure, with a local `surfaceState` that forwards changes to the optional parent callback:

```svelte
<Dialog.Root {open} onOpenChange={changeOpen}>
	<Dialog.Content
		class="flex h-[94dvh] w-[96vw] max-w-[96vw] flex-col overflow-hidden p-3"
		aria-busy={surfaceState === 'loading'}
	>
		<Dialog.Header class="shrink-0 px-2 pt-1">
			<Dialog.Title>{title}</Dialog.Title>
			<Dialog.Description>{description}</Dialog.Description>
		</Dialog.Header>
		<div class="min-h-0 flex-1 py-2">
			<CertificatePreviewSurface
				{manifest}
				{manifestLoading}
				{manifestError}
				{ariaLabel}
				{loadingLabel}
				{renderFailureMessage}
				{retryLabel}
				{onretry}
				onstatechange={handleSurfaceStateChange}
			/>
		</div>
		<Dialog.Footer class="shrink-0">
			<Button variant="secondary" disabled={!manifest} onclick={() => (fullscreenOpen = true)}>
				ขยายเต็มจอ
			</Button>
			<Button variant="outline" onclick={() => changeOpen(false)}>ปิด</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<CertificatePreviewFullscreenDialog
	open={fullscreenOpen}
	{title}
	{manifest}
	{manifestLoading}
	{manifestError}
	{ariaLabel}
	{loadingLabel}
	{renderFailureMessage}
	{retryLabel}
	{onretry}
	onopenchange={(nextOpen) => (fullscreenOpen = nextOpen)}
/>
```

`changeOpen(false)` sets `fullscreenOpen = false` before forwarding `onopenchange(false)`. `handleSurfaceStateChange(nextState)` assigns `surfaceState`, then calls `onstatechange?.(nextState)`. Passing the same manifest and retry callback to `CertificatePreviewFullscreenDialog` means `Escape` closes only the nested fullscreen dialog and returns focus to the normal preview.

- [ ] **Step 5: Run the editor architecture assertion and verify the private renderer still fails it**

Run:

```bash
cd frontend-school
node --test --test-name-pattern='editor real PDF preview' tests/static/certificate-editor.test.mjs
```

Expected: FAIL because `CertificateEditor.svelte` still contains its private dialog/canvas renderer.

- [ ] **Step 6: Refactor the editor to acquire manifests while the shared dialog renders them**

Replace `previewCanvas`, direct renderer import, and private dialog markup with:

```ts
let previewManifest = $state.raw<CertificateRenderManifest | null>(null);
let previewManifestLoading = $state(false);
let previewManifestError = $state('');
let previewState = $state<CertificatePreviewState>('idle');

async function renderPreview(kind: 'short' | 'normal' | 'long' | 'candidate') {
	if (previewState === 'loading' || kind === 'candidate') return;
	previewController?.abort();
	const controller = new AbortController();
	previewController = controller;
	lastPreviewKind = kind;
	previewOpen = true;
	previewManifest = null;
	previewManifestLoading = true;
	previewManifestError = '';
	try {
		const freshManifest = await createCertificateTemplatePreviewManifest(
			currentTemplate.id,
			{ previewKind: kind, layout: cloneCertificateLayout(layout) },
			{ signal: controller.signal }
		);
		controller.signal.throwIfAborted();
		manifest = freshManifest;
		previewManifest = freshManifest;
	} catch (error) {
		if (controller.signal.aborted || previewController !== controller) return;
		previewManifestError =
			error instanceof Error ? error.message : 'สร้างพรีวิวไม่สำเร็จ';
	} finally {
		if (previewController === controller) {
			previewController = null;
			previewManifestLoading = false;
		}
	}
}
```

Pass `onstatechange={(state) => (previewState = state)}` to the shared dialog so the toolbar remains disabled until rendering completes. Retry calls `renderPreview(lastPreviewKind)` after confirming the last kind exists; close aborts, nulls the manifest, clears errors, and restores `idle`.

Replace the old private `<Dialog.Root>` block with:

```svelte
<CertificatePreviewDialog
	open={previewOpen}
	title="พรีวิว PDF จริง"
	description="ใช้ renderer เดียวกับไฟล์ดาวน์โหลด รวมฟอนต์ไทย เงา รูปภาพ และ QR Code"
	manifest={previewManifest}
	manifestLoading={previewManifestLoading}
	manifestError={previewManifestError}
	ariaLabel="ผลพรีวิว PDF จริง"
	loadingLabel="กำลังโหลดฟอนต์และสร้างตัวอย่าง…"
	renderFailureMessage="สร้างพรีวิว PDF จริงไม่สำเร็จ"
	onretry={retryPreview}
	onstatechange={(state) => (previewState = state)}
	onopenchange={(open) => !open && closePreview()}
/>
```

- [ ] **Step 7: Run the Svelte autofixer sequentially**

Run each command separately and resolve every reported issue:

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/CertificatePreviewSurface.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/CertificatePreviewFullscreenDialog.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/CertificatePreviewDialog.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/editor/CertificateEditor.svelte --svelte-version 5
```

Expected: no unresolved issues for any file.

- [ ] **Step 8: Run focused static tests**

Run:

```bash
cd frontend-school
node --test tests/static/certificate-preview.test.mjs
```

Expected: all fit and shared-source tests PASS.

Then run:

```bash
cd frontend-school
node --test tests/static/certificate-editor.test.mjs
```

Expected: all editor static tests PASS.

- [ ] **Step 9: Run the editor browser workflow serially**

Run:

```bash
cd frontend-school
npx playwright test tests/e2e/certificate-editor.spec.ts --workers=1
```

Expected: editor tests PASS, including loading, fit, retry, fullscreen, and `Escape`.

- [ ] **Step 10: Commit the shared viewer and editor integration**

```bash
git add frontend-school/src/lib/certificates/preview-fit.ts frontend-school/src/lib/components/certificates/CertificatePreviewSurface.svelte frontend-school/src/lib/components/certificates/CertificatePreviewFullscreenDialog.svelte frontend-school/src/lib/components/certificates/CertificatePreviewDialog.svelte frontend-school/src/lib/components/certificates/editor/CertificateEditor.svelte frontend-school/tests/static/certificate-preview.test.mjs frontend-school/tests/static/certificate-editor.test.mjs frontend-school/tests/e2e/certificate-editor.spec.ts
git commit -m "refactor(certificates): share real preview viewer"
```

---

### Task 3: Staff Issuance-Request Preview Integration

**Files:**
- Modify: `frontend-school/src/lib/components/certificates/CertificateIssueRequestReview.svelte`
- Modify: `frontend-school/tests/static/certificate-request-ui.test.mjs`
- Modify: `frontend-school/tests/e2e/certificate-request-workflow.spec.ts`

**Interfaces:**
- Consumes: `CertificatePreviewDialog` and `CertificatePreviewState` from Tasks 1–2; existing candidate manifest endpoint.
- Produces: a candidate preview that fits the actual dialog at wide desktop viewports and preserves request-review authorization and copy.

- [ ] **Step 1: Make the staff request tests reproduce the overflow**

Extend the request harness renderer stub so it sizes the canvas from the supplied scale and records calls:

```ts
const rendererStub = `
	export async function loadCertificateRenderer() {
		return {
			async renderPreview(manifest, canvas, options = {}) {
				options.signal?.throwIfAborted();
				const scale = options.scale ?? 1;
				canvas.width = Math.round(manifest.pageGeometry.displayedWidthPoints * scale);
				canvas.height = Math.round(manifest.pageGeometry.displayedHeightPoints * scale);
				window.__certificateRequestPreviewRenders += 1;
				return {
					widthPoints: manifest.pageGeometry.displayedWidthPoints,
					heightPoints: manifest.pageGeometry.displayedHeightPoints,
					widthPixels: canvas.width,
					heightPixels: canvas.height
				};
			}
		};
	}
`;
```

Update the API stub signature to forward cancellation:

```ts
export async function createCertificateTemplatePreviewManifest(templateId, payload, options = {}) {
	return window.__certificateRequestApi.preview(templateId, payload, options);
}
```

Return an A4 portrait candidate manifest from `window.__certificateRequestApi.preview`, call `options.signal?.throwIfAborted()` before returning, record `templateId`, `previewKind`, and `candidateId`, and initialize `window.__certificateRequestPreviewRenders = 0`:

```ts
const previewCalls = [];
window.__certificateRequestPreviewRenders = 0;

function candidateManifest(templateId) {
	return {
		templateId,
		certificateNumber: 'PREVIEW',
		suggestedFilename: 'preview.pdf',
		layout: { schemaVersion: 1, elements: [] },
		pageGeometry: {
			paperLabel: 'A4 แนวตั้ง',
			rotation: 0,
			displayedWidthPoints: 595,
			displayedHeightPoints: 842,
			mediaBox: { xPoints: 0, yPoints: 0, widthPoints: 595, heightPoints: 842 },
			cropBox: { xPoints: 0, yPoints: 0, widthPoints: 595, heightPoints: 842 }
		},
		backgroundGrant: {
			fileId: '51000000-0000-4000-8000-000000000001',
			url: '/candidate-background.pdf',
			expiresAt: '2099-01-01T00:00:00Z'
		},
		fontGrants: [],
		imageGrants: [],
		builtInFonts: [],
		qrPayload: 'candidate-preview',
		recipientValues: { ชื่อ: 'กมลชนก', นามสกุล: 'ใจดี' },
		campaignValues: {
			academicYear: '2569',
			campaignName: 'กิจกรรมวันภาษาไทย',
			eventDate: '2026-08-01',
			issueDate: '2026-08-14',
			ownerOrganizationUnitName: 'กลุ่มสาระภาษาไทย',
			schoolName: 'โรงเรียนตัวอย่าง'
		}
	};
}

window.__certificateRequestApi.preview = async (templateId, payload, options = {}) => {
	options.signal?.throwIfAborted();
	previewCalls.push({ templateId, ...structuredClone(payload) });
	return candidateManifest(templateId);
};
```

Add this Playwright test:

```ts
test('candidate preview fits the real review dialog without horizontal scrolling', async ({ page }) => {
	await page.setViewportSize({ width: 1920, height: 1080 });
	await page.goto(`${baseUrl}${harnessPath}?view=review`);
	await page.getByRole('button', { name: 'เริ่มตรวจคำขอ' }).click();
	await page.getByRole('button', { name: 'ออกเกียรติบัตร 2 ใบ' }).click();
	await page.getByRole('button', { name: 'ดูตัวอย่างแบบนี้' }).first().click();

	const previewDialog = page.getByRole('dialog', { name: 'ตัวอย่างเกียรติบัตร' });
	await expect(previewDialog).toBeVisible();
	const stage = previewDialog.getByTestId('certificate-preview-stage');
	const canvas = stage.getByLabel('ตัวอย่างเกียรติบัตรสำหรับตรวจคำขอ');
	await expect(canvas).toBeVisible();
	await expect
		.poll(() => page.evaluate(() => window.certificateRequestHarness.previewCalls().at(-1)))
		.toEqual({
			templateId: '30000000-0000-4000-8000-000000000001',
			previewKind: 'candidate',
			candidateId: '40000000-0000-4000-8000-000000000001'
		});
	const metrics = await stage.evaluate((node) => {
		const paper = node.querySelector('canvas');
		if (!(paper instanceof HTMLCanvasElement)) throw new Error('preview canvas missing');
		const outer = node.getBoundingClientRect();
		const inner = paper.getBoundingClientRect();
		return {
			overflow: node.scrollWidth > node.clientWidth + 1,
			inside: inner.left >= outer.left - 1 && inner.right <= outer.right + 1
		};
	});
expect(metrics).toEqual({ overflow: false, inside: true });
});
```

Expose `previewCalls: () => structuredClone(previewCalls)` on `window.certificateRequestHarness` and add its exact return type to the test's global `Window` declaration.

Add static assertions:

```js
assert.match(review, /CertificatePreviewDialog/);
assert.doesNotMatch(review, /window\.innerWidth/);
assert.doesNotMatch(review, /max-w-none/);
```

- [ ] **Step 2: Run the focused staff E2E and verify the old dialog fails**

Run:

```bash
cd frontend-school
npx playwright test tests/e2e/certificate-request-workflow.spec.ts --workers=1 --grep='candidate preview fits'
```

Expected: FAIL because the old canvas overflows the `sm:max-w-5xl` dialog and has no shared loading/fullscreen surface.

- [ ] **Step 3: Replace the staff review's private canvas renderer with the shared dialog**

Keep manifest acquisition in `CertificateIssueRequestReview.svelte` and replace direct renderer/canvas state with:

```ts
let previewManifest = $state.raw<CertificateRenderManifest | null>(null);
let previewManifestLoading = $state(false);
let previewManifestError = $state('');
let previewState = $state<CertificatePreviewState>('idle');

async function preview(item: CertificateIssueRequestItem) {
	if (!canIssue || previewState === 'loading' || !item.templateId) return;
	previewController?.abort();
	const controller = new AbortController();
	previewController = controller;
	previewItem = item;
	previewOpen = true;
	previewManifest = null;
	previewManifestLoading = true;
	previewManifestError = '';
	try {
		const manifest = await createCertificateTemplatePreviewManifest(
			item.templateId,
			{ previewKind: 'candidate', candidateId: item.candidateId },
			{ signal: controller.signal }
		);
		controller.signal.throwIfAborted();
		previewManifest = manifest;
	} catch (error) {
		if (controller.signal.aborted || previewController !== controller) return;
		previewManifestError =
			error instanceof Error ? error.message : 'สร้างตัวอย่างไม่สำเร็จ';
	} finally {
		if (previewController === controller) {
			previewController = null;
			previewManifestLoading = false;
		}
	}
}
```

Retry calls `preview(previewItem)` after resetting `previewState` from `error` to `idle`. Close aborts the controller, clears item/manifest/errors, and sets `idle`.

Render `CertificatePreviewDialog` with:

```svelte
<CertificatePreviewDialog
	open={previewOpen}
	title="ตัวอย่างเกียรติบัตร"
	description={`${previewItem ? displayName(previewItem) : ''} · ข้อมูลพรีวิว ไม่ใช่เกียรติบัตรที่ออกเลขแล้ว`}
	manifest={previewManifest}
	manifestLoading={previewManifestLoading}
	manifestError={previewManifestError}
	ariaLabel="ตัวอย่างเกียรติบัตรสำหรับตรวจคำขอ"
	loadingLabel="กำลังโหลดฟอนต์และสร้างตัวอย่าง…"
	renderFailureMessage="สร้างตัวอย่างเกียรติบัตรไม่สำเร็จ"
	onretry={retryPreview}
	onstatechange={(state) => (previewState = state)}
	onopenchange={(open) => !open && closePreview()}
/>
```

- [ ] **Step 4: Run the Svelte autofixer for the staff review**

Run:

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/CertificateIssueRequestReview.svelte --svelte-version 5
```

Expected: no unresolved issues.

- [ ] **Step 5: Run staff static and E2E tests sequentially**

Run:

```bash
cd frontend-school
node --test tests/static/certificate-request-ui.test.mjs
```

Expected: all request UI static tests PASS.

Then run:

```bash
cd frontend-school
npx playwright test tests/e2e/certificate-request-workflow.spec.ts --workers=1
```

Expected: all request workflow tests PASS, including the wide-viewport fit case.

- [ ] **Step 6: Commit the staff preview integration**

```bash
git add frontend-school/src/lib/components/certificates/CertificateIssueRequestReview.svelte frontend-school/tests/static/certificate-request-ui.test.mjs frontend-school/tests/e2e/certificate-request-workflow.spec.ts
git commit -m "fix(certificates): fit issuance request previews"
```

---

### Task 4: Automatic Public Preview Data Flow and Safety Gates

**Files:**
- Modify: `frontend-school/src/lib/components/certificates/PublicCertificateVerification.svelte`
- Modify: `frontend-school/tests/static/certificate-public-verification.test.mjs`
- Modify: `frontend-school/tests/e2e/certificate-public-verification.spec.ts`

**Interfaces:**
- Consumes: `CertificatePreviewSurface`, existing public verification APIs, `ApiClientError`, and the public render receipt.
- Produces: `VerificationContext` held only in memory, parent-owned public preview manifest state, one explicit receipt-refresh attempt on retry, and automatic issued preview for both manual and QR entry.

- [ ] **Step 1: Extend the public harness with real preview behavior**

Update the renderer stub to implement both preview and PDF paths:

```ts
const rendererStub = `
	export async function loadCertificateRenderer() {
		window.__certificatePublicRendererLoads += 1;
		return {
			renderPreview: async (manifest, canvas, options = {}) => {
				options.signal?.throwIfAborted();
				await window.__certificatePublicPreviewControl.beforeRender(options.signal);
				options.signal?.throwIfAborted();
				const scale = options.scale ?? 1;
				canvas.width = Math.round(manifest.pageGeometry.displayedWidthPoints * scale);
				canvas.height = Math.round(manifest.pageGeometry.displayedHeightPoints * scale);
				window.__certificatePublicPreviews.push(manifest.certificateNumber);
				return {
					widthPoints: manifest.pageGeometry.displayedWidthPoints,
					heightPoints: manifest.pageGeometry.displayedHeightPoints,
					widthPixels: canvas.width,
					heightPixels: canvas.height
				};
			},
			buildCertificatePdf: async (manifests) => {
				window.__certificatePublicBuilds.push(manifests.map((item) => item.certificateNumber));
				return new Uint8Array([37, 80, 68, 70]);
			}
		};
	}
`;
```

Add a public API-client stub and register it in `stubModules`:

```ts
const apiClientStub = `
	export class ApiClientError extends Error {
		constructor(message, status) {
			super(message);
			this.name = 'ApiClientError';
			this.status = status;
		}
	}
`;
```

Import that class inside the virtual harness module. Add deterministic renderer gates and API modes:

```ts
let failPreviewCount = mode === 'preview-error' ? 1 : 0;
let holdNextPreview = mode === 'loading' || mode === 'stale';
let releaseHeldPreview = null;
let verificationAttempt = 0;
let manifestAttempt = 0;

window.__certificatePublicPreviewControl = {
	async beforeRender() {
		if (failPreviewCount > 0) {
			failPreviewCount -= 1;
			throw new Error('controlled preview failure');
		}
		if (!holdNextPreview) return;
		holdNextPreview = false;
		await new Promise((resolve) => {
			releaseHeldPreview = resolve;
		});
	},
	release() {
		const release = releaseHeldPreview;
		releaseHeldPreview = null;
		release?.();
	}
};

window.__certificatePublicApi.verifyManual = async (payload) => {
	verificationAttempt += 1;
	verificationCalls.push({
		kind: 'manual',
		payload: structuredClone(payload),
		hashAtCall: window.location.hash
	});
	return result(
		mode === 'revoked-after-expiry' && verificationAttempt > 1 ? 'revoked' : 'issued'
	);
};

window.__certificatePublicApi.render = async (payload) => {
	manifestAttempt += 1;
	renderCalls.push(structuredClone(payload));
	if (
		(mode === 'expired' || mode === 'revoked-after-expiry') &&
		manifestAttempt === 1
	) {
		throw new ApiClientError('ไม่พบข้อมูลที่ตรงกัน', 404);
	}
	return manifest();
};
```

Expose `releaseHeldPreview: () => window.__certificatePublicPreviewControl.release()` and preview-call snapshots on `window.certificatePublicHarness`. The `expired` mode returns issued on both verification calls and throws status `404` only for the first manifest call; `revoked-after-expiry` returns issued first, throws `404` for that receipt, then returns revoked on re-verification; `preview-error` fails only the first canvas render while leaving PDF building successful.

Change the manual issued test so it expects one automatic render-manifest call before clicking download, one visible canvas labelled `ภาพเกียรติบัตรที่ตรวจสอบแล้ว`, and a second manifest call for the independent PDF download. Extend the QR test with the same automatic-preview assertion. Keep the revoked test's exact zero-call assertions.

Import `type Page` from Playwright and add one reusable form helper before the tests:

```ts
async function completeManualVerification(
	page: Page,
	firstName = 'กมลชนก',
	lastName = 'ใจดี'
): Promise<void> {
	await page
		.getByRole('textbox', { name: 'เลขเกียรติบัตร', exact: true })
		.fill('2569-0042-000123-4');
	await page.getByLabel('ชื่อ', { exact: true }).fill(firstName);
	await page.getByLabel('นามสกุล').fill(lastName);
	await page.getByRole('button', { name: 'ตรวจสอบข้อมูล' }).click();
}
```

Add focused tests for loading, receipt refresh, and independent download:

```ts
test('issued public preview reports font and render progress', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=loading`);
	await completeManualVerification(page);
	await expect(page.getByText('กำลังสร้างภาพเกียรติบัตร…')).toBeVisible();
	await page.evaluate(() => window.certificatePublicHarness.releaseHeldPreview());
	await expect(page.getByLabel('ภาพเกียรติบัตรที่ตรวจสอบแล้ว')).toBeVisible();
});

test('issued preview shows progress and retry re-verifies one expired receipt', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=expired`);
	await page.getByRole('textbox', { name: 'เลขเกียรติบัตร', exact: true }).fill('2569-0042-000123-4');
	await page.getByLabel('ชื่อ', { exact: true }).fill('กมลชนก');
	await page.getByLabel('นามสกุล').fill('ใจดี');
	await page.getByRole('button', { name: 'ตรวจสอบข้อมูล' }).click();
	await expect(page.getByText('สร้างภาพเกียรติบัตรไม่สำเร็จ')).toBeVisible();
	await page.getByRole('button', { name: 'ลองโหลดภาพอีกครั้ง' }).click();
	await expect(page.getByLabel('ภาพเกียรติบัตรที่ตรวจสอบแล้ว')).toBeVisible();
	await expect
		.poll(() => page.evaluate(() => window.certificatePublicHarness.verificationCalls().length))
		.toBe(2);
});

test('preview failure keeps verified details and PDF download usable', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=preview-error`);
	await completeManualVerification(page, 'กมลชนก', 'ใจดี');
	const result = page.getByTestId('verification-result');
	await expect(result).toContainText('ใช้ได้');
	await expect(result).toContainText('กมลชนก ใจดี');
	await expect(page.getByText('สร้างภาพเกียรติบัตรไม่สำเร็จ')).toBeVisible();
	await page.getByRole('button', { name: 'ดาวน์โหลดเกียรติบัตร' }).click();
	await expect
		.poll(() => page.evaluate(() => window.certificatePublicHarness.downloads()))
		.toEqual([{ byteLength: 4, filename: '2569-0042-000123-4.pdf' }]);
});

test('receipt retry that discovers revocation removes preview and download', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=revoked-after-expiry`);
	await completeManualVerification(page);
	await expect(page.getByText('สร้างภาพเกียรติบัตรไม่สำเร็จ')).toBeVisible();
	await page.getByRole('button', { name: 'ลองโหลดภาพอีกครั้ง' }).click();
	await expect(page.getByTestId('verification-result')).toContainText('เพิกถอนแล้ว');
	await expect(page.getByRole('button', { name: 'ดาวน์โหลดเกียรติบัตร' })).toHaveCount(0);
	await expect(page.getByLabel('ภาพเกียรติบัตรที่ตรวจสอบแล้ว')).toHaveCount(0);
});
```

- [ ] **Step 2: Run the public E2E and verify automatic preview assertions fail**

Run:

```bash
cd frontend-school
npx playwright test tests/e2e/certificate-public-verification.spec.ts --workers=1
```

Expected: FAIL because successful verification currently renders only text and requests a manifest only after download.

- [ ] **Step 3: Add ephemeral verification and preview state**

Import the shared surface, `CertificatePreviewState`, `ApiClientError`, and request/manifest types. Add this discriminated context and state:

```ts
type VerificationContext =
	| { kind: 'manual'; payload: ManualCertificateVerificationRequest }
	| { kind: 'qr'; payload: QrCertificateVerificationRequest };

let verificationContext = $state.raw<VerificationContext | null>(null);
let previewManifest = $state.raw<CertificateRenderManifest | null>(null);
let previewManifestLoading = $state(false);
let previewManifestError = $state('');
let previewState = $state<CertificatePreviewState>('idle');
let previewController: AbortController | null = null;
```

Refactor manual and QR verification to create a context, scrub the QR fragment before creating the QR context, and pass it to one `runVerification(context)` function. That function aborts the old verification, preview, and download; clears the old manifest immediately; calls the matching typed API; stores the successful context only in memory; sets the verified result; and starts `void loadPublicPreview(verified, false)` only for issued results with a receipt.

Use these helpers for exact branching:

```ts
function verifyContext(
	context: VerificationContext,
	signal: AbortSignal
): Promise<PublicCertificateVerificationData> {
	return context.kind === 'manual'
		? verifyCertificateManually(context.payload, { signal })
		: verifyCertificateByQr(context.payload, { signal });
}

async function loadPublicPreview(
	verified: PublicCertificateVerificationData,
	allowReceiptRefresh: boolean
): Promise<void> {
	if (verified.status !== 'issued' || !verified.receipt) return;
	const initialReceipt = verified.receipt;
	const contextSnapshot = verificationContext;
	previewController?.abort();
	const controller = new AbortController();
	previewController = controller;
	previewManifest = null;
	previewManifestLoading = true;
	previewManifestError = '';
	try {
		let manifest: CertificateRenderManifest;
		try {
			manifest = await createPublicCertificateRenderManifest(
				{ receipt: initialReceipt },
				{ signal: controller.signal }
			);
		} catch (error) {
			if (
				controller.signal.aborted ||
				!allowReceiptRefresh ||
				!(error instanceof ApiClientError) ||
				error.status !== 404 ||
				!contextSnapshot
			) {
				throw error;
			}
			const refreshed = await verifyContext(contextSnapshot, controller.signal);
			result = refreshed;
			if (refreshed.status !== 'issued' || !refreshed.receipt) {
				previewManifest = null;
				return;
			}
			manifest = await createPublicCertificateRenderManifest(
				{ receipt: refreshed.receipt },
				{ signal: controller.signal }
			);
		}
		controller.signal.throwIfAborted();
		previewManifest = manifest;
	} catch {
		if (controller.signal.aborted || previewController !== controller) return;
		previewManifestError = 'สร้างภาพเกียรติบัตรไม่สำเร็จ';
	} finally {
		if (previewController === controller) {
			previewController = null;
			previewManifestLoading = false;
		}
	}
}

function retryPublicPreview(): void {
	if (result?.status !== 'issued' || !result.receipt || previewManifestLoading) return;
	void loadPublicPreview(result, true);
}
```

The inner refresh branch runs at most once per button action. Do not loop, persist `verificationContext`, or display the caught error.

- [ ] **Step 4: Embed the shared surface for issued results and preserve independent download**

Within the issued result branch add:

```svelte
<CertificatePreviewSurface
	manifest={previewManifest}
	manifestLoading={previewManifestLoading}
	manifestError={previewManifestError}
	ariaLabel="ภาพเกียรติบัตรที่ตรวจสอบแล้ว"
	loadingLabel="กำลังสร้างภาพเกียรติบัตร…"
	renderFailureMessage="สร้างภาพเกียรติบัตรไม่สำเร็จ"
	retryLabel="ลองโหลดภาพอีกครั้ง"
	onretry={retryPublicPreview}
	onstatechange={(state) => (previewState = state)}
/>
```

Use the button label `ลองโหลดภาพอีกครั้ง` for this public surface. Keep `downloadCertificate` as a separate fresh manifest request and separate controller/state. Abort and clear preview state before every new verification and in `onMount` cleanup. Do not instantiate the surface in the revoked branch.

- [ ] **Step 5: Strengthen public static guards**

Add source assertions that require the shared surface and issued gate while forbidding persistent QR proof storage:

```js
assert.match(component, /CertificatePreviewSurface/);
assert.match(component, /status\s*!==\s*['"]issued['"][\s\S]*!verified\.receipt/);
assert.match(component, /loadPublicPreview\(verified, false\)/);
assert.doesNotMatch(component, /(?:localStorage|sessionStorage)\./);
assert.doesNotMatch(component, /console\.(?:log|debug|info|warn|error)/);
```

Retain the E2E zero-call assertions as the authoritative proof that revoked results do not request a manifest or renderer.

- [ ] **Step 6: Run the Svelte autofixer**

Run:

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/PublicCertificateVerification.svelte --svelte-version 5
```

Expected: no unresolved issues.

- [ ] **Step 7: Run public static and browser tests sequentially**

Run:

```bash
cd frontend-school
node --test tests/static/certificate-public-verification.test.mjs
```

Expected: all public verification static tests PASS.

Then run:

```bash
cd frontend-school
npx playwright test tests/e2e/certificate-public-verification.spec.ts --workers=1
```

Expected: manual, QR, loading/retry, download, and revoked tests PASS.

- [ ] **Step 8: Commit automatic public preview behavior**

```bash
git add frontend-school/src/lib/components/certificates/PublicCertificateVerification.svelte frontend-school/tests/static/certificate-public-verification.test.mjs frontend-school/tests/e2e/certificate-public-verification.spec.ts
git commit -m "feat(certificates): render verified public certificates"
```

---

### Task 5: Public Registry Result Layout, Fullscreen, and Stale-Result Protection

**Files:**
- Modify: `frontend-school/src/lib/components/certificates/PublicCertificateVerification.svelte`
- Modify: `frontend-school/tests/e2e/certificate-public-verification.spec.ts`

**Interfaces:**
- Consumes: public manifest state from Task 4 and `CertificatePreviewFullscreenDialog` from Task 2.
- Produces: results-oriented desktop grid, status-first mobile order, `ตรวจสอบหมายเลขอื่น`, fullscreen, and stale-preview browser guarantees.

- [ ] **Step 1: Add failing responsive, fullscreen, and stale-result browser assertions**

Extend the harness so manual verification derives the returned recipient from the submitted fields while preserving the Task 4 receipt/revocation modes:

```ts
function manualResult(status, payload) {
	return {
		...result(status),
		firstName: payload.firstName,
		lastName: payload.lastName
	};
}

window.__certificatePublicApi.verifyManual = async (payload) => {
	verificationAttempt += 1;
	verificationCalls.push({
		kind: 'manual',
		payload: structuredClone(payload),
		hashAtCall: window.location.hash
	});
	const status =
		mode === 'revoked-after-expiry' && verificationAttempt > 1 ? 'revoked' : 'issued';
	return manualResult(status, payload);
};
```

The `stale` mode already holds only the first preview render through Task 4's `holdNextPreview` gate. Add these tests:

```ts
test('issued registry layout keeps preview primary on desktop and status first on mobile', async ({ page }) => {
	await page.setViewportSize({ width: 1440, height: 900 });
	await page.goto(`${baseUrl}${harnessPath}?mode=manual`);
	await completeManualVerification(page, 'กมลชนก', 'ใจดี');
	const preview = page.getByTestId('public-certificate-preview-region');
	const details = page.getByTestId('public-certificate-details');
	await expect(preview).toBeVisible();
	expect((await preview.boundingBox())?.x).toBeLessThan((await details.boundingBox())?.x ?? 0);

	await page.setViewportSize({ width: 390, height: 844 });
	const status = page.getByTestId('public-certificate-status');
	expect((await status.boundingBox())?.y).toBeLessThan((await preview.boundingBox())?.y ?? 0);
	expect((await preview.boundingBox())?.y).toBeLessThan((await details.boundingBox())?.y ?? 0);
});

test('public fullscreen exits with Escape and returns to the verified result', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=manual`);
	await completeManualVerification(page, 'กมลชนก', 'ใจดี');
	await page.getByRole('button', { name: 'ขยายเต็มจอ' }).click();
	const fullscreen = page.getByRole('dialog', { name: 'เกียรติบัตรที่ตรวจสอบแล้วแบบเต็มจอ' });
	await expect(fullscreen).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(fullscreen).toBeHidden();
	await expect(page.getByTestId('verification-result')).toBeVisible();
});

test('a new verification clears and aborts the previous certificate preview', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=stale`);
	await completeManualVerification(page, 'คนแรก', 'ทดสอบ');
	await expect(page.getByText('กำลังสร้างภาพเกียรติบัตร…')).toBeVisible();
	await page.getByRole('button', { name: 'ตรวจสอบหมายเลขอื่น' }).click();
	await completeManualVerification(page, 'คนที่สอง', 'ทดสอบ');
	await page.evaluate(() => window.certificatePublicHarness.releaseHeldPreview());
	await expect(page.getByTestId('verification-result')).toContainText('คนที่สอง');
	await expect(page.getByTestId('verification-result')).not.toContainText('คนแรก');
});
```

Reuse the `completeManualVerification` helper created in Task 4; do not add a second copy.

- [ ] **Step 2: Run the new public layout tests and verify they fail**

Run:

```bash
cd frontend-school
npx playwright test tests/e2e/certificate-public-verification.spec.ts --workers=1 --grep='issued registry layout|public fullscreen|new verification clears'
```

Expected: FAIL because the current result remains in the old form/result split and has no shared fullscreen or reset action.

- [ ] **Step 3: Reshape the successful issued result around the certificate**

Keep the initial form/error workspace. When `result` exists, render a dedicated result workspace instead of the form split:

```svelte
{#if result}
	<section
		class:revoked-result={result.status === 'revoked'}
		class="verified-registry"
		data-testid="verification-result"
	>
		<div class:revoked={result.status === 'revoked'} class="status-seal" data-testid="public-certificate-status">
			{#if result.status === 'issued'}
				<ShieldCheck size={31} aria-hidden="true" />
				<div><span>สถานะ</span><strong>ใช้ได้</strong></div>
			{:else}
				<ShieldX size={31} aria-hidden="true" />
				<div><span>สถานะ</span><strong>เพิกถอนแล้ว</strong></div>
			{/if}
		</div>

		{#if result.status === 'issued' && result.receipt}
			<div class="certificate-preview-region" data-testid="public-certificate-preview-region">
				<CertificatePreviewSurface
					manifest={previewManifest}
					manifestLoading={previewManifestLoading}
					manifestError={previewManifestError}
					ariaLabel="ภาพเกียรติบัตรที่ตรวจสอบแล้ว"
					loadingLabel="กำลังสร้างภาพเกียรติบัตร…"
					renderFailureMessage="สร้างภาพเกียรติบัตรไม่สำเร็จ"
					retryLabel="ลองโหลดภาพอีกครั้ง"
					onretry={retryPublicPreview}
				/>
			</div>
		{/if}

		<div class="certificate-details" data-testid="public-certificate-details">
			<div class="certificate-number">
				<span>เลขเกียรติบัตร</span>
				<strong>{result.certificateNumber}</strong>
			</div>
			<div class="recipient">
				<span>มอบให้</span>
				<h2>{recipientName}</h2>
			</div>
			<dl>
				<div><dt>กิจกรรม</dt><dd>{result.campaignName}</dd></div>
				<div><dt>แบบเกียรติบัตร</dt><dd>{result.templateName}</dd></div>
				{#if result.activityItem}
					<div><dt>รายการ</dt><dd>{result.activityItem}</dd></div>
				{/if}
				{#if result.awardOrRole}
					<div><dt>รางวัลหรือบทบาท</dt><dd>{result.awardOrRole}</dd></div>
				{/if}
				<div>
					<dt>วันที่ออก</dt>
					<dd>{formatThaiDate(result.issueDate)} · ปีการศึกษา {result.academicYear}</dd>
				</div>
				<div><dt>ผู้ออก</dt><dd>{result.issuerSchoolName}</dd></div>
			</dl>
		</div>

		<div class="result-actions">
			{#if result.status === 'issued' && result.receipt}
				<button type="button" class="download-button" onclick={downloadCertificate} disabled={downloading}>
					{downloading ? 'กำลังสร้าง PDF' : 'ดาวน์โหลดเกียรติบัตร'}
				</button>
				<button
					type="button"
					class="secondary-action"
					disabled={previewState !== 'ready'}
					onclick={() => (previewFullscreenOpen = true)}
				>
					ขยายเต็มจอ
				</button>
			{:else if result.status === 'revoked'}
				<div class="revoked-note">
					<p>เกียรติบัตรฉบับนี้ถูกเพิกถอนและไม่สามารถดาวน์โหลดได้</p>
					{#if result.replacementCertificateNumber}
						<span>เลขใบทดแทน: <strong>{result.replacementCertificateNumber}</strong></span>
					{/if}
				</div>
			{/if}
			<button type="button" class="secondary-action" onclick={resetVerification}>
				ตรวจสอบหมายเลขอื่น
			</button>
			{#if downloadError}<p class="download-error">{downloadError}</p>{/if}
		</div>
	</section>
{:else}
	<div class="workspace">
		<section class="manual-panel" aria-labelledby="manual-title">
			<form data-testid="certificate-verification-form" onsubmit={submitManualVerification}>
				<label for="certificate-number">เลขเกียรติบัตร</label>
				<input id="certificate-number" bind:value={certificateNumber} />
				<div class="name-fields">
					<label for="first-name">ชื่อ<input id="first-name" bind:value={firstName} /></label>
					<label for="last-name">นามสกุล<input id="last-name" bind:value={lastName} /></label>
				</div>
				<button class="verify-button" type="submit" disabled={verifying}>
					{verifying ? 'กำลังตรวจสอบ' : 'ตรวจสอบข้อมูล'}
				</button>
			</form>
		</section>
		<section class="result-panel" aria-live="polite" aria-busy={verifying}>
			{#if verifying}
				<div class="result-placeholder"><p>กำลังตรวจสอบทะเบียน</p></div>
			{:else if verificationError}
				<div class="result-placeholder error-state" data-testid="verification-error">
					<h2>ตรวจสอบไม่สำเร็จ</h2><p>{verificationError}</p>
				</div>
			{:else}
				<div class="result-placeholder"><h2>ผลการตรวจสอบจะแสดงที่นี่</h2></div>
			{/if}
		</section>
	</div>
{/if}
```

Preserve the current field attributes, icons, privacy note, helper copy, and autocomplete values when moving the existing form into the `{:else}` branch; the abbreviated form markup above defines the branch structure, not permission to remove those existing accessibility details.

Keep the existing real registry fields; do not replace text details with canvas-only content. Implement `resetVerification()` to abort verification/preview/download, increment the request sequence, clear result/errors/context/manifest/input fields, close fullscreen, and return to the manual form.

- [ ] **Step 4: Apply the approved responsive registry layout and tokens**

Expand the successful shell to `width: min(1440px, 100%)`. Use named grid areas so desktop and mobile order differ without duplicating content:

```css
.verified-registry {
	display: grid;
	grid-template-columns: minmax(0, 1.65fr) minmax(20rem, 0.65fr);
	grid-template-areas:
		'preview status'
		'preview details'
		'preview actions';
	align-items: start;
	border-top: 1px solid var(--registry-line);
}

.certificate-preview-region {
	grid-area: preview;
	min-width: 0;
	height: min(72dvh, 54rem);
	min-height: 28rem;
	padding: clamp(0.8rem, 2vw, 1.5rem);
	background: var(--registry-mist);
	border-right: 1px solid var(--registry-line);
}

.status-seal { grid-area: status; }
.certificate-details { grid-area: details; }
.result-actions { grid-area: actions; }

.verified-registry.revoked-result {
	grid-template-columns: minmax(0, 44rem);
	grid-template-areas:
		'status'
		'details'
		'actions';
	justify-content: center;
}

@media (max-width: 800px) {
	.verified-registry {
		grid-template-columns: minmax(0, 1fr);
		grid-template-areas:
			'status'
			'preview'
			'details'
			'actions';
	}

	.certificate-preview-region {
		height: min(65dvh, 36rem);
		min-height: 18rem;
		border-right: 0;
		border-block: 1px solid var(--registry-line);
	}
}
```

Preserve the approved registry ink/blue/mist/line/gold, verified green, revoked red, and monospaced certificate number. Do not add gradients, decorative stock imagery, or unrelated animation.

- [ ] **Step 5: Add public fullscreen with the shared dialog**

Add `let previewFullscreenOpen = $state(false)` and render:

```svelte
<CertificatePreviewFullscreenDialog
	open={previewFullscreenOpen}
	title="เกียรติบัตรที่ตรวจสอบแล้ว"
	manifest={previewManifest}
	manifestLoading={previewManifestLoading}
	manifestError={previewManifestError}
	ariaLabel="ภาพเกียรติบัตรที่ตรวจสอบแล้วแบบเต็มจอ"
	loadingLabel="กำลังสร้างภาพเกียรติบัตร…"
	renderFailureMessage="สร้างภาพเกียรติบัตรไม่สำเร็จ"
	retryLabel="ลองโหลดภาพอีกครั้ง"
	onretry={retryPublicPreview}
	onopenchange={(open) => (previewFullscreenOpen = open)}
/>
```

Render its `ขยายเต็มจอ` opener only for an issued result with a ready manifest. The accessible dialog primitive owns focus trapping and `Escape`.

- [ ] **Step 6: Run the Svelte autofixer**

Run:

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/PublicCertificateVerification.svelte --svelte-version 5
```

Expected: no unresolved issues.

- [ ] **Step 7: Run all public verification browser tests serially**

Run:

```bash
cd frontend-school
npx playwright test tests/e2e/certificate-public-verification.spec.ts --workers=1
```

Expected: all public verification tests PASS, including desktop/mobile order, fullscreen, stale cancellation, QR, retry, download, and revoked zero calls.

- [ ] **Step 8: Run public static tests**

Run:

```bash
cd frontend-school
node --test tests/static/certificate-public-verification.test.mjs
```

Expected: all public verification static tests PASS.

- [ ] **Step 9: Commit the public result design**

```bash
git add frontend-school/src/lib/components/certificates/PublicCertificateVerification.svelte frontend-school/tests/e2e/certificate-public-verification.spec.ts
git commit -m "feat(certificates): reshape public verification results"
```

---

### Task 6: Sequential Final Verification and Diff Review

**Files:**
- Verify only: every file changed in Tasks 1–5.

**Interfaces:**
- Consumes: all preceding commits.
- Produces: evidence that focused behavior, Svelte analysis, the frontend verification matrix, and repository hygiene pass on the final tree.

- [ ] **Step 1: Run every changed Svelte file through the autofixer, one command at a time**

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/CertificatePreviewSurface.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/CertificatePreviewFullscreenDialog.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/CertificatePreviewDialog.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/editor/CertificateEditor.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/CertificateIssueRequestReview.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer src/lib/components/certificates/PublicCertificateVerification.svelte --svelte-version 5
```

Expected: every command reports no unresolved issue.

- [ ] **Step 2: Run focused static tests one file at a time**

```bash
cd frontend-school
node --test tests/static/certificate-preview.test.mjs
node --test tests/static/certificate-editor.test.mjs
node --test tests/static/certificate-request-ui.test.mjs
node --test tests/static/certificate-public-verification.test.mjs
```

Expected: every file passes independently.

- [ ] **Step 3: Run focused Playwright files one at a time with one worker**

```bash
cd frontend-school
npx playwright test tests/e2e/certificate-editor.spec.ts --workers=1
npx playwright test tests/e2e/certificate-request-workflow.spec.ts --workers=1
npx playwright test tests/e2e/certificate-public-verification.spec.ts --workers=1
```

Expected: every Playwright file passes independently.

- [ ] **Step 4: Run the required frontend matrix sequentially**

Run:

```bash
cd frontend-school
npm run lint
```

Expected: Prettier and ESLint PASS.

Then run:

```bash
cd frontend-school
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: SvelteKit sync and `svelte-check` PASS with 0 errors.

Then run:

```bash
cd frontend-school
npm run test:static
```

Expected: the full static suite PASS.

- [ ] **Step 5: Review repository hygiene**

Run:

```bash
git diff --check
```

Expected: no output.

Then run:

```bash
git status --short
```

Expected: no uncommitted implementation files. Do not create an empty verification commit; if a correction is required, return to the task that owns it, add a focused regression test, make the correction, rerun that task's checks, and commit the correction with a scoped message.

- [ ] **Step 6: Review the final commits and scope**

Run:

```bash
git log --oneline --decorate -8
```

Expected: the spec/plan commits followed by focused fit, shared viewer/editor, staff preview, public preview, and public layout commits.

Then run:

```bash
git diff origin/main...HEAD --stat
```

Expected: changes are limited to the approved spec/plan and frontend certificate preview/test files; no backend, migration, permission, or generated API artifact appears.
