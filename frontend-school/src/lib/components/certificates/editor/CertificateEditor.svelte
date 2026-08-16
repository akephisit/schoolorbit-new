<script lang="ts">
	import { beforeNavigate } from '$app/navigation';
	import {
		createCertificateTemplatePreviewManifest,
		getCertificateTemplate,
		getCertificateTemplateVariableCatalog,
		updateCertificateTemplate,
		type CertificateRenderManifest,
		type CertificateTemplateDetail
	} from '$lib/api/certificates';
	import { ApiClientError } from '$lib/api/client';
	import {
		alignElements,
		cloneCertificateLayout,
		constrainElementToPage,
		createImageElement,
		createQrElement,
		createTextElement,
		duplicateElement,
		fitEditorZoom,
		imageAssetAspectRatio,
		reorderElement,
		stepEditorZoom,
		type CertificateElement,
		type CertificateLayout,
		type ElementAlignment,
		type LayerDirection
	} from '$lib/certificates/editor-state';
	import { pointsToMillimetres, millimetresToPoints } from '$lib/certificates/layout';
	import { loadCertificateRenderer } from '$lib/certificates/renderer';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { AlertTriangle, RefreshCw, ShieldAlert } from 'lucide-svelte';
	import { onDestroy, tick, untrack } from 'svelte';
	import { toast } from 'svelte-sonner';
	import CertificateBackgroundReplaceDialog from './CertificateBackgroundReplaceDialog.svelte';
	import CertificateCanvas from './CertificateCanvas.svelte';
	import CertificateElementPanel from './CertificateElementPanel.svelte';
	import CertificateLayersPanel from './CertificateLayersPanel.svelte';
	import CertificateToolbar from './CertificateToolbar.svelte';

	let {
		template,
		initialManifest,
		variables
	}: {
		template: CertificateTemplateDetail;
		initialManifest: CertificateRenderManifest;
		variables: string[];
	} = $props();

	let currentTemplate = $state.raw(untrack(() => template));
	let manifest = $state.raw(untrack(() => initialManifest));
	let variableOptions = $state.raw(untrack(() => [...variables]));
	let layout = $state.raw(cloneCertificateLayout(untrack(() => template.layout)));
	let selectedIds = $state.raw<string[]>([]);
	let zoom = $state(1);
	let safeMarginPoints = $state(untrack(() => template.safeMarginPoints));
	let showSafeArea = $state(untrack(() => template.showSafeArea));
	let snapToGuides = $state(true);
	let dirty = $state(false);
	let saving = $state(false);
	let conflictError = $state('');
	let missingValueWarning = $state('');
	let reloading = $state(false);
	let backgroundOpen = $state(false);
	let backgroundPending = $state(false);
	let previewOpen = $state(false);
	let previewing = $state<'short' | 'normal' | 'long' | 'candidate' | null>(null);
	let previewError = $state('');
	let previewCanvas = $state<HTMLCanvasElement>();
	let previewController: AbortController | null = null;
	let manifestRefreshPromise: Promise<CertificateRenderManifest> | null = null;

	const canEdit = $derived(currentTemplate.capabilities.canUpdate && !saving && !reloading);
	const selectedElement = $derived.by(() => {
		if (selectedIds.length !== 1) return null;
		return layout.elements.find((element) => element.id === selectedIds[0]) ?? null;
	});
	const hasQr = $derived(layout.elements.some((element) => element.type === 'qr'));
	const geometry = $derived(currentTemplate.pageGeometry ?? manifest.pageGeometry);
	const pageSize = $derived({
		width: geometry.displayedWidthPoints,
		height: geometry.displayedHeightPoints
	});
	const safeMarginMillimetres = $derived(
		Math.round(pointsToMillimetres(safeMarginPoints) * 10) / 10
	);

	function setLayout(next: CertificateLayout) {
		if (!canEdit) return;
		layout = next;
		dirty = true;
		conflictError = '';
		missingValueWarning = '';
	}

	function setSelection(elementIds: string[]) {
		selectedIds = Array.from(new Set(elementIds)).filter((id) =>
			layout.elements.some((element) => element.id === id)
		);
	}

	function selectElement(elementId: string, additive: boolean) {
		setSelection(
			additive
				? selectedIds.includes(elementId)
					? selectedIds.filter((id) => id !== elementId)
					: [...selectedIds, elementId]
				: [elementId]
		);
	}

	function addText() {
		const element = createTextElement(pageSize);
		setLayout({ schemaVersion: 1, elements: [...layout.elements, element] });
		setSelection([element.id]);
	}

	function addQr() {
		if (hasQr) {
			toast.error('หนึ่งแบบมี QR Code ได้หนึ่งรายการ');
			return;
		}
		const element = createQrElement(pageSize);
		setLayout({ schemaVersion: 1, elements: [...layout.elements, element] });
		setSelection([element.id]);
	}

	function addImage(assetId: string) {
		const asset = currentTemplate.assets.find(
			(candidate) => candidate.id === assetId && candidate.kind === 'image'
		);
		if (!asset) {
			toast.error('ไม่พบข้อมูลรูปภาพที่เลือก กรุณาโหลดหน้าใหม่');
			return;
		}
		let aspectRatio: number;
		try {
			aspectRatio = imageAssetAspectRatio(asset);
		} catch {
			toast.error('รูปภาพนี้ไม่มีข้อมูลขนาดต้นฉบับ กรุณาอัปโหลดใหม่');
			return;
		}
		const element = createImageElement(pageSize, assetId, aspectRatio);
		setLayout({ schemaVersion: 1, elements: [...layout.elements, element] });
		setSelection([element.id]);
	}

	function patchElement(updated: CertificateElement) {
		const constrained = constrainElementToPage(updated, pageSize);
		setLayout({
			schemaVersion: 1,
			elements: layout.elements.map((element) =>
				element.id === constrained.id ? constrained : element
			)
		});
	}

	function deleteSelected() {
		if (selectedIds.length === 0) return;
		const selected = new Set(selectedIds);
		setLayout({
			schemaVersion: 1,
			elements: layout.elements.filter((element) => !selected.has(element.id))
		});
		setSelection([]);
	}

	function duplicateSelected() {
		const selected = layout.elements.filter((element) => selectedIds.includes(element.id));
		const duplicable = selected.filter((element) => element.type !== 'qr');
		if (duplicable.length === 0) {
			toast.error('QR Code มีได้หนึ่งรายการ จึงทำสำเนาไม่ได้');
			return;
		}
		const copies = duplicable.map((element) =>
			constrainElementToPage(duplicateElement(element), pageSize)
		);
		setLayout({ schemaVersion: 1, elements: [...layout.elements, ...copies] });
		setSelection(copies.map((element) => element.id));
	}

	function reorder(elementId: string, direction: LayerDirection) {
		setLayout({
			schemaVersion: 1,
			elements: reorderElement(layout.elements, elementId, direction)
		});
	}

	function align(alignment: ElementAlignment) {
		if (selectedIds.length < 2) return;
		setLayout({
			schemaVersion: 1,
			elements: alignElements(layout.elements, selectedIds, alignment).map((element) =>
				selectedIds.includes(element.id) ? constrainElementToPage(element, pageSize) : element
			)
		});
	}

	function toggleSafeArea() {
		if (!canEdit) return;
		showSafeArea = !showSafeArea;
		dirty = true;
	}

	function updateSafeMargin(event: Event) {
		const millimetres = Number((event.currentTarget as HTMLInputElement).value);
		if (!Number.isFinite(millimetres)) return;
		safeMarginPoints = millimetresToPoints(Math.max(0, Math.min(50, millimetres)));
		dirty = true;
	}

	function fitCanvas() {
		const availableWidth = Math.max(420, window.innerWidth - 660);
		const availableHeight = Math.max(360, window.innerHeight - 220);
		zoom = fitEditorZoom(availableWidth, availableHeight, pageSize.width, pageSize.height, 72);
	}

	async function saveLayout(confirmMissingIssuedValues = false) {
		if (!canEdit || saving || !dirty) return;
		saving = true;
		conflictError = '';
		missingValueWarning = '';
		try {
			const updated = await updateCertificateTemplate(currentTemplate.id, {
				expectedUpdatedAt: currentTemplate.updatedAt,
				layout,
				safeMarginPoints,
				showSafeArea,
				confirmMissingIssuedValues
			});
			currentTemplate = updated;
			layout = cloneCertificateLayout(updated.layout);
			safeMarginPoints = updated.safeMarginPoints;
			showSafeArea = updated.showSafeArea;
			dirty = false;
			toast.success('บันทึกการจัดวางแล้ว');
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				if (error.message.includes('ตัวแปรว่าง')) missingValueWarning = error.message;
				else conflictError = error.message;
				return;
			}
			toast.error(error instanceof Error ? error.message : 'บันทึกการจัดวางไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	async function reloadServerCopy() {
		if (reloading) return;
		reloading = true;
		try {
			const [updated, catalog, updatedManifest] = await Promise.all([
				getCertificateTemplate(currentTemplate.id),
				getCertificateTemplateVariableCatalog(currentTemplate.id),
				createCertificateTemplatePreviewManifest(currentTemplate.id, {
					previewKind: 'short'
				})
			]);
			currentTemplate = updated;
			manifest = updatedManifest;
			variableOptions = catalog.variables;
			layout = cloneCertificateLayout(updated.layout);
			safeMarginPoints = updated.safeMarginPoints;
			showSafeArea = updated.showSafeArea;
			selectedIds = [];
			dirty = false;
			conflictError = '';
			missingValueWarning = '';
			toast.success('โหลดสำเนาล่าสุดจากระบบแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'โหลดข้อมูลล่าสุดไม่สำเร็จ');
		} finally {
			reloading = false;
		}
	}

	async function renderPreview(kind: 'short' | 'normal' | 'long' | 'candidate') {
		if (previewing || kind === 'candidate') return;
		previewController?.abort();
		const controller = new AbortController();
		previewController = controller;
		previewing = kind;
		previewError = '';
		previewOpen = true;
		try {
			const layoutSnapshot = cloneCertificateLayout(layout);
			const freshManifest = await createCertificateTemplatePreviewManifest(currentTemplate.id, {
				previewKind: kind,
				layout: layoutSnapshot
			});
			controller.signal.throwIfAborted();
			manifest = freshManifest;
			await tick();
			if (!previewCanvas) throw new Error('ไม่พบพื้นที่แสดงพรีวิว');
			const renderer = await loadCertificateRenderer();
			const previewScale = Math.min(
				1.5,
				Math.max(
					0.35,
					Math.min(
						(window.innerWidth - 120) / freshManifest.pageGeometry.displayedWidthPoints,
						(window.innerHeight - 220) / freshManifest.pageGeometry.displayedHeightPoints
					)
				)
			);
			await renderer.renderPreview(freshManifest, previewCanvas, {
				scale: previewScale,
				signal: controller.signal
			});
		} catch (error) {
			if (controller.signal.aborted) return;
			previewError = error instanceof Error ? error.message : 'สร้างพรีวิวไม่สำเร็จ';
		} finally {
			if (previewController === controller) {
				previewController = null;
				previewing = null;
			}
		}
	}

	async function refreshCanvasManifest(): Promise<CertificateRenderManifest> {
		if (manifestRefreshPromise) return manifestRefreshPromise;
		const layoutSnapshot = cloneCertificateLayout(layout);
		let request!: Promise<CertificateRenderManifest>;
		request = createCertificateTemplatePreviewManifest(currentTemplate.id, {
			previewKind: 'short',
			layout: layoutSnapshot
		})
			.then((freshManifest) => {
				manifest = freshManifest;
				return freshManifest;
			})
			.finally(() => {
				if (manifestRefreshPromise === request) manifestRefreshPromise = null;
			});
		manifestRefreshPromise = request;
		return request;
	}

	async function handleBackgroundPatched(updated: CertificateTemplateDetail) {
		currentTemplate = updated;
		layout = cloneCertificateLayout(updated.layout);
		safeMarginPoints = updated.safeMarginPoints;
		showSafeArea = updated.showSafeArea;
		selectedIds = [];
		dirty = false;
		conflictError = '';
		missingValueWarning = '';
		try {
			manifest = await createCertificateTemplatePreviewManifest(updated.id, {
				previewKind: 'short'
			});
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'โหลดพื้นหลังใหม่ไม่สำเร็จ');
		}
	}

	function closePreview() {
		previewController?.abort();
		previewController = null;
		previewing = null;
		previewOpen = false;
	}

	beforeNavigate(({ cancel }) => {
		if (!dirty && !backgroundPending) return;
		cancel();
		toast.error(
			backgroundPending
				? 'แนบหรือลบไฟล์ชั่วคราวให้เสร็จก่อนออกจาก editor'
				: 'บันทึกหรือยกเลิกการแก้ไขก่อนออกจาก editor'
		);
	});

	onDestroy(() => previewController?.abort());
</script>

<div
	class="flex h-[calc(100dvh-7rem)] min-h-[42rem] flex-col overflow-hidden rounded-xl border bg-background shadow-sm"
	data-testid="certificate-editor"
>
	<CertificateToolbar
		templateName={currentTemplate.name}
		{dirty}
		{saving}
		canSave={currentTemplate.capabilities.canUpdate}
		{zoom}
		{showSafeArea}
		{snapToGuides}
		selectionCount={selectedIds.length}
		{previewing}
		editingDisabled={!canEdit}
		backgroundDisabled={dirty}
		onsave={() => saveLayout(false)}
		onzoom={(direction) => (zoom = stepEditorZoom(zoom, direction))}
		onfit={fitCanvas}
		onsafetoggle={toggleSafeArea}
		onsnapchange={(enabled) => (snapToGuides = enabled)}
		onpreview={renderPreview}
		onalign={align}
		onduplicate={duplicateSelected}
		ondelete={deleteSelected}
		onbackground={() => {
			if (!canEdit) return;
			if (dirty) {
				toast.error('บันทึกการจัดวางก่อนเปลี่ยนพื้นหลัง');
				return;
			}
			backgroundOpen = true;
		}}
	/>

	{#if conflictError}
		<div
			class="flex flex-wrap items-center gap-3 border-b border-destructive/25 bg-destructive/5 px-4 py-2 text-xs"
		>
			<AlertTriangle class="size-4 shrink-0 text-destructive" />
			<p class="min-w-0 flex-1">
				<strong>สำเนาบนระบบเปลี่ยนแล้ว:</strong>
				{conflictError} การจัดวางในหน้าจอนี้ยังคงอยู่
			</p>
			<Button size="sm" variant="outline" disabled={reloading} onclick={reloadServerCopy}>
				<RefreshCw class={`size-3.5 ${reloading ? 'animate-spin' : ''}`} /> โหลดสำเนาระบบ
			</Button>
		</div>
	{/if}

	{#if missingValueWarning}
		<div
			class="flex flex-wrap items-center gap-3 border-b border-amber-300 bg-amber-50 px-4 py-2 text-xs text-amber-950"
		>
			<ShieldAlert class="size-4 shrink-0" />
			<p class="min-w-0 flex-1">{missingValueWarning}</p>
			<Button size="sm" variant="outline" disabled={saving} onclick={() => saveLayout(true)}>
				ยืนยันบันทึกแม้ข้อมูลเดิมไม่ครบ
			</Button>
		</div>
	{/if}

	<div class="grid min-h-0 min-w-[72rem] flex-1 grid-cols-[17rem_minmax(32rem,1fr)_18rem]">
		<aside class="min-h-0 overflow-y-auto border-r bg-background p-4">
			<CertificateElementPanel
				{selectedElement}
				assets={currentTemplate.assets}
				variables={variableOptions}
				{hasQr}
				disabled={!canEdit}
				onaddtext={addText}
				onaddqr={addQr}
				onaddimage={addImage}
				onpatch={patchElement}
				onduplicate={duplicateSelected}
				ondelete={deleteSelected}
			/>
		</aside>

		<main class="min-h-0 min-w-0 overflow-hidden bg-slate-100">
			<CertificateCanvas
				template={currentTemplate}
				{layout}
				{manifest}
				{selectedIds}
				{zoom}
				{safeMarginPoints}
				{showSafeArea}
				{snapToGuides}
				disabled={!canEdit}
				onmanifestrefresh={refreshCanvasManifest}
				onlayoutchange={setLayout}
				onselectionchange={setSelection}
				onduplicate={duplicateSelected}
				ondelete={deleteSelected}
			/>
		</main>

		<aside class="min-h-0 space-y-5 overflow-y-auto border-l bg-background p-4">
			<section
				class="space-y-3 rounded-xl border bg-muted/15 p-3"
				aria-labelledby="safe-area-settings"
			>
				<div>
					<h2 id="safe-area-settings" class="text-xs font-semibold">พื้นที่ปลอดภัย</h2>
					<p class="mt-0.5 text-[0.68rem] leading-relaxed text-muted-foreground">
						เส้นเตือนช่วยกันข้อความชิดขอบ แต่ไม่พิมพ์ลงใน PDF
					</p>
				</div>
				<label class="space-y-1 text-[0.7rem]">
					<span>ระยะขอบ (มม.)</span>
					<Input
						type="number"
						min="0"
						max="50"
						step="0.5"
						value={safeMarginMillimetres}
						disabled={!canEdit}
						onchange={updateSafeMargin}
					/>
				</label>
			</section>
			<CertificateLayersPanel
				elements={layout.elements}
				{selectedIds}
				disabled={!canEdit}
				onselect={selectElement}
				onreorder={reorder}
			/>
		</aside>
	</div>
</div>

<CertificateBackgroundReplaceDialog
	bind:open={backgroundOpen}
	template={currentTemplate}
	previewManifest={{ ...manifest, layout: cloneCertificateLayout(layout) }}
	onmanifestrefresh={refreshCanvasManifest}
	onpatched={handleBackgroundPatched}
	onpendingchange={(pending) => (backgroundPending = pending)}
/>

<Dialog.Root bind:open={previewOpen} onOpenChange={(open) => !open && closePreview()}>
	<Dialog.Content class="max-h-[96vh] overflow-auto p-3 sm:max-w-[96vw]">
		<Dialog.Header class="px-2 pt-2">
			<Dialog.Title>พรีวิว PDF จริง</Dialog.Title>
			<Dialog.Description>
				ใช้ renderer เดียวกับไฟล์ดาวน์โหลด รวมฟอนต์ไทย เงา รูปภาพ และ QR Code
			</Dialog.Description>
		</Dialog.Header>
		<div class="grid min-h-72 place-items-center overflow-auto rounded-lg bg-slate-200 p-5">
			{#if previewError}
				<div
					class="max-w-md rounded-lg border border-destructive/30 bg-background p-4 text-center text-sm text-destructive"
				>
					<AlertTriangle class="mx-auto mb-2 size-5" />{previewError}
				</div>
			{:else}
				<canvas bind:this={previewCanvas} class="max-w-none bg-white shadow-xl"></canvas>
			{/if}
		</div>
	</Dialog.Content>
</Dialog.Root>
