<script lang="ts">
	import { onDestroy } from 'svelte';
	import {
		attachCertificateTemplateBackground,
		type AttachCertificateBackgroundRequest,
		type CertificateRenderManifest,
		type CertificateTemplateDetail
	} from '$lib/api/certificates';
	import { deleteFile, uploadCertificateTemplateFile, type FileMetadata } from '$lib/api/files';
	import {
		certificateManifestExpiresSoon,
		certificatePageGeometryMatches,
		cloneCertificateLayout,
		resetCertificateLayout,
		scaleCertificateLayout
	} from '$lib/certificates/editor-state';
	import {
		loadCertificateRenderer,
		type CertificateBackgroundInspection
	} from '$lib/certificates/renderer';
	import { LoadingButton } from '$lib/components/app-state';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { AlertTriangle, Eye, FileCheck2, RefreshCw, Trash2, Upload } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	type GeometryAction = AttachCertificateBackgroundRequest['geometryAction'];

	let {
		template,
		previewManifest,
		onmanifestrefresh,
		onpatched,
		onpendingchange
	}: {
		template: CertificateTemplateDetail;
		previewManifest?: CertificateRenderManifest;
		onmanifestrefresh?: () => Promise<CertificateRenderManifest>;
		onpatched: (template: CertificateTemplateDetail) => void;
		onpendingchange: (pending: boolean) => void;
	} = $props();

	let selectedFile = $state<File | null>(null);
	let unattachedFile = $state.raw<FileMetadata | null>(null);
	let geometryAction = $state<GeometryAction>('preserve');
	let previewConfirmed = $state(false);
	let previewUrl = $state('');
	let previewCanvas = $state<HTMLCanvasElement>();
	let inspectedGeometry = $state.raw<CertificateBackgroundInspection | null>(null);
	let exactPreviewReady = $state(false);
	let exactPreviewError = $state('');
	let attachError = $state<Error | null>(null);
	let uploading = $state(false);
	let cleaning = $state(false);
	let fileInputKey = $state(0);
	let previewController: AbortController | null = null;

	const geometryChanged = $derived(
		inspectedGeometry && previewManifest
			? !certificatePageGeometryMatches(previewManifest.pageGeometry, inspectedGeometry)
			: null
	);
	const replacementNeedsExactPreview = $derived(template.backgroundFileId !== null);
	const replacementReady = $derived(
		!replacementNeedsExactPreview ||
			(previewManifest !== undefined &&
				exactPreviewReady &&
				geometryChanged !== null &&
				(!geometryChanged || (geometryAction !== 'preserve' && previewConfirmed)))
	);

	const actionLabels: Record<GeometryAction, string> = {
		preserve: 'รักษาตำแหน่งเดิมเมื่อขนาดเท่ากัน',
		scale: 'ปรับองค์ประกอบตามสัดส่วนหน้าใหม่',
		reset: 'ล้างองค์ประกอบและเริ่มจัดวางใหม่'
	};

	function asError(error: unknown, fallback: string): Error {
		return error instanceof Error ? error : new Error(fallback);
	}

	function setUnattachedFile(file: FileMetadata | null) {
		unattachedFile = file;
		onpendingchange(file !== null);
	}

	function clearPreviewUrl() {
		previewController?.abort();
		previewController = null;
		if (previewUrl) URL.revokeObjectURL(previewUrl);
		previewUrl = '';
	}

	function resetExactPreview() {
		previewConfirmed = false;
		exactPreviewReady = false;
		exactPreviewError = '';
		inspectedGeometry = null;
	}

	function selectFile(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		selectedFile = input.files?.[0] ?? null;
		geometryAction = 'preserve';
		attachError = null;
		resetExactPreview();
		clearPreviewUrl();
		if (selectedFile) previewUrl = URL.createObjectURL(selectedFile);
	}

	function changeGeometryAction(value: string | undefined) {
		if (value !== 'preserve' && value !== 'scale' && value !== 'reset') return;
		geometryAction = value;
		previewConfirmed = false;
		exactPreviewReady = false;
		exactPreviewError = '';
	}

	$effect(() => {
		const file = selectedFile;
		const url = previewUrl;
		const canvas = previewCanvas;
		const currentManifest = previewManifest;
		const refreshManifest = onmanifestrefresh;
		const action = geometryAction;
		if (!file || !url || !canvas || !currentManifest || template.backgroundFileId === null) return;

		const controller = new AbortController();
		previewController?.abort();
		previewController = controller;
		exactPreviewReady = false;
		exactPreviewError = '';
		canvas.width = 1;
		canvas.height = 1;
		void (async () => {
			let baseManifest = currentManifest;
			if (certificateManifestExpiresSoon(baseManifest)) {
				if (!refreshManifest) {
					throw new Error('สิทธิ์อ่านรูปหรือฟอนต์หมดอายุ กรุณาปิดแล้วเปิด editor ใหม่');
				}
				baseManifest = await refreshManifest();
				controller.signal.throwIfAborted();
			}
			const renderer = await loadCertificateRenderer();
			const nextGeometry = await renderer.inspectBackgroundPdf(file, controller.signal);
			controller.signal.throwIfAborted();
			inspectedGeometry = nextGeometry;
			const changed = !certificatePageGeometryMatches(baseManifest.pageGeometry, nextGeometry);
			if (!changed && action !== 'preserve') {
				geometryAction = 'preserve';
				return;
			}
			if (changed && action === 'preserve') return;
			const nextLayout = changed
				? action === 'scale'
					? scaleCertificateLayout(
							baseManifest.layout,
							{
								width: baseManifest.pageGeometry.displayedWidthPoints,
								height: baseManifest.pageGeometry.displayedHeightPoints
							},
							{
								width: nextGeometry.displayedWidthPoints,
								height: nextGeometry.displayedHeightPoints
							}
						)
					: resetCertificateLayout()
				: cloneCertificateLayout(baseManifest.layout);
			const scale = Math.min(
				1.25,
				Math.max(
					0.35,
					Math.min(
						620 / nextGeometry.displayedWidthPoints,
						440 / nextGeometry.displayedHeightPoints
					)
				)
			);
			await renderer.renderPreview(
				{
					...baseManifest,
					pageGeometry: nextGeometry,
					layout: nextLayout,
					backgroundGrant: { ...baseManifest.backgroundGrant, url }
				},
				canvas,
				{ scale, signal: controller.signal }
			);
			controller.signal.throwIfAborted();
			exactPreviewReady = true;
		})().catch((error: unknown) => {
			if (controller.signal.aborted) return;
			exactPreviewError =
				error instanceof Error ? error.message : 'ไม่สามารถสร้างตัวอย่างพื้นหลังใหม่ได้';
		});

		return () => controller.abort();
	});

	async function attachUploadedFile() {
		if (!unattachedFile) return;
		try {
			const updated = await attachCertificateTemplateBackground(template.id, {
				fileId: unattachedFile.id,
				geometryAction: geometryChanged === false ? 'preserve' : geometryAction,
				previewConfirmed: template.backgroundFileId === null ? false : previewConfirmed
			});
			setUnattachedFile(null);
			selectedFile = null;
			attachError = null;
			resetExactPreview();
			geometryAction = 'preserve';
			fileInputKey += 1;
			clearPreviewUrl();
			onpatched(updated);
			toast.success(
				template.backgroundFileId ? 'เปลี่ยน PDF พื้นหลังแล้ว' : 'แนบ PDF พื้นหลังแล้ว'
			);
		} catch (error) {
			attachError = asError(error, 'แนบ PDF พื้นหลังไม่สำเร็จ');
		}
	}

	async function uploadAndAttach() {
		if (uploading || (!selectedFile && !unattachedFile) || !replacementReady) return;
		uploading = true;
		onpendingchange(true);
		attachError = null;
		try {
			if (!unattachedFile && selectedFile) {
				setUnattachedFile(
					await uploadCertificateTemplateFile(
						selectedFile,
						'certificate_template_background',
						template.id
					)
				);
			}
			await attachUploadedFile();
		} catch (error) {
			attachError = asError(error, 'อัปโหลด PDF พื้นหลังไม่สำเร็จ');
		} finally {
			uploading = false;
			if (!unattachedFile) onpendingchange(false);
		}
	}

	async function deleteTemporaryUpload() {
		if (!unattachedFile || cleaning) return;
		cleaning = true;
		try {
			await deleteFile(unattachedFile.id, template.id);
			setUnattachedFile(null);
			selectedFile = null;
			attachError = null;
			resetExactPreview();
			fileInputKey += 1;
			clearPreviewUrl();
			toast.success('ลบไฟล์ชั่วคราวแล้ว');
		} catch (error) {
			attachError = asError(error, 'ลบไฟล์ชั่วคราวไม่สำเร็จ');
		} finally {
			cleaning = false;
		}
	}

	onDestroy(clearPreviewUrl);
</script>

<section class="space-y-4" aria-labelledby={`background-title-${template.id}`}>
	<div class="flex flex-wrap items-start justify-between gap-3">
		<div>
			<h3 id={`background-title-${template.id}`} class="font-medium">PDF พื้นหลัง</h3>
			<p class="mt-1 text-xs leading-relaxed text-muted-foreground">
				PDF หนึ่งหน้าเป็นแหล่งจริงของขนาดและแนวกระดาษ ระบบจะอ่าน geometry หลังอัปโหลด
			</p>
		</div>
		{#if template.backgroundFileId}
			<span
				class="inline-flex items-center gap-1.5 rounded-full border border-emerald-200 bg-emerald-50 px-2.5 py-1 text-xs font-medium text-emerald-700"
			>
				<FileCheck2 class="size-3.5" /> แนบแล้ว
			</span>
		{/if}
	</div>

	{#if attachError}
		<div class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm">
			<div class="flex items-start gap-2 text-destructive">
				<AlertTriangle class="mt-0.5 size-4 shrink-0" />
				<p>{attachError.message}</p>
			</div>
			{#if unattachedFile}
				<p class="mt-2 text-xs text-muted-foreground">
					ไฟล์ผ่านการอัปโหลดแล้วแต่ยังไม่ถูกแนบ คุณเปลี่ยนวิธีจัดวางแล้วลองซ้ำ หรือลบไฟล์ชั่วคราวได้
				</p>
				<div class="mt-3 flex flex-wrap gap-2">
					<LoadingButton size="sm" variant="outline" loading={uploading} onclick={uploadAndAttach}>
						<RefreshCw class="size-4" /> ลองแนบอีกครั้ง
					</LoadingButton>
					<LoadingButton
						size="sm"
						variant="outline"
						loading={cleaning}
						onclick={deleteTemporaryUpload}
					>
						<Trash2 class="size-4" /> ลบไฟล์ชั่วคราว
					</LoadingButton>
				</div>
			{/if}
		</div>
	{/if}

	<div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(15rem,0.65fr)]">
		<div class="space-y-4">
			<div class="space-y-2">
				<Label for={`certificate-background-${template.id}`}>
					{template.backgroundFileId ? 'เลือก PDF ใหม่' : 'เลือก PDF พื้นหลัง'}
				</Label>
				{#key fileInputKey}
					<Input
						id={`certificate-background-${template.id}`}
						type="file"
						accept=".pdf"
						onchange={selectFile}
						disabled={!template.capabilities.canUpdate || unattachedFile !== null || uploading}
					/>
				{/key}
			</div>

			{#if template.backgroundFileId}
				<div class="space-y-2">
					<Label for={`geometry-action-${template.id}`}>เมื่อขนาดหน้าใหม่ต่างจากเดิม</Label>
					<Select.Root
						type="single"
						value={geometryAction}
						onValueChange={changeGeometryAction}
						disabled={geometryChanged === false}
					>
						<Select.Trigger id={`geometry-action-${template.id}`} class="w-full">
							{actionLabels[geometryAction]}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="preserve">{actionLabels.preserve}</Select.Item>
							<Select.Item value="scale">{actionLabels.scale}</Select.Item>
							<Select.Item value="reset">{actionLabels.reset}</Select.Item>
						</Select.Content>
					</Select.Root>
					<p class="text-xs text-muted-foreground">
						หากเลือก “รักษาตำแหน่งเดิม” แต่ geometry ต่างกัน
						ระบบจะไม่เปลี่ยนพื้นหลังและจะแจ้งให้เลือกใหม่
					</p>
					{#if selectedFile && previewManifest === undefined}
						<p class="text-xs font-medium text-destructive">
							เปิด editor เพื่อเปลี่ยนพื้นหลังและตรวจผลการจัดวางจริงก่อนยืนยัน
						</p>
					{:else if selectedFile && geometryChanged === true && geometryAction === 'preserve'}
						<p class="text-xs font-medium text-amber-700">
							ขนาดหรือการหมุนของหน้าเปลี่ยนแล้ว โปรดเลือก “ปรับองค์ประกอบตามสัดส่วน” หรือ
							“ล้างองค์ประกอบ”
						</p>
					{:else if selectedFile && geometryChanged === false}
						<p class="text-xs font-medium text-emerald-700">
							geometry เท่ากับพื้นหลังเดิม ระบบจะรักษาตำแหน่งทุกองค์ประกอบ
						</p>
					{/if}
				</div>
			{/if}

			{#if selectedFile && geometryChanged === true && geometryAction !== 'preserve'}
				<label
					class={[
						'flex items-start gap-3 rounded-lg border border-blue-200 bg-blue-50 p-3 text-sm text-blue-950',
						exactPreviewReady ? 'cursor-pointer' : 'cursor-not-allowed opacity-70'
					]}
				>
					<Checkbox bind:checked={previewConfirmed} disabled={!exactPreviewReady} class="mt-0.5" />
					<span>
						<strong class="font-medium">
							{exactPreviewReady
								? 'ตรวจผลพรีวิวจริงและยืนยันวิธีจัดวางแล้ว'
								: 'รอสร้างผลพรีวิวจริงก่อนยืนยัน'}
						</strong>
						<span class="mt-0.5 block text-xs text-blue-800">
							การเปลี่ยนพื้นหลังมีผลต่อ PDF ที่สร้างใหม่ของใบเดิมด้วย
						</span>
					</span>
				</label>
			{/if}

			<LoadingButton
				loading={uploading}
				disabled={!template.capabilities.canUpdate ||
					(!selectedFile && !unattachedFile) ||
					!replacementReady}
				onclick={uploadAndAttach}
			>
				<Upload class="size-4" />
				{template.backgroundFileId ? 'เปลี่ยนพื้นหลัง' : 'แนบพื้นหลัง'}
			</LoadingButton>
		</div>

		<div class="overflow-hidden rounded-xl border bg-muted/30">
			{#if previewUrl && previewManifest && template.backgroundFileId}
				<div class="flex items-center gap-2 border-b bg-background px-3 py-2 text-xs font-medium">
					<Eye class="size-3.5" /> ผลพรีวิวจริงหลังเปลี่ยนพื้นหลัง
				</div>
				<div class="relative grid min-h-64 place-items-center overflow-auto bg-slate-200 p-4">
					<canvas
						bind:this={previewCanvas}
						class={['max-w-none bg-white shadow-lg', exactPreviewReady ? '' : 'opacity-30']}
						aria-label="ผลพรีวิวเกียรติบัตรหลังเปลี่ยนพื้นหลัง"
					></canvas>
					{#if exactPreviewError}
						<div
							class="absolute inset-0 grid place-items-center p-5 text-center text-sm text-destructive"
						>
							<div><AlertTriangle class="mx-auto mb-2 size-5" />{exactPreviewError}</div>
						</div>
					{:else if geometryChanged === true && geometryAction === 'preserve'}
						<p
							class="absolute inset-0 grid place-items-center p-5 text-center text-sm text-amber-800"
						>
							เลือกวิธีจัดวางสำหรับ geometry ใหม่เพื่อสร้างพรีวิว
						</p>
					{:else if !exactPreviewReady}
						<p
							class="absolute inset-0 grid place-items-center p-5 text-center text-sm text-muted-foreground"
						>
							กำลังตรวจ PDF และสร้างผลลัพธ์ด้วย renderer จริง…
						</p>
					{/if}
				</div>
			{:else if previewUrl}
				<div class="flex items-center gap-2 border-b bg-background px-3 py-2 text-xs font-medium">
					<Eye class="size-3.5" /> ตัวอย่างไฟล์ที่เลือก
				</div>
				<object
					data={previewUrl}
					type="application/pdf"
					class="h-52 w-full"
					aria-label="ตัวอย่าง PDF"
				>
					<p class="p-4 text-xs text-muted-foreground">เบราว์เซอร์นี้ไม่รองรับตัวอย่าง PDF</p>
				</object>
			{:else}
				<div class="grid min-h-52 place-items-center p-5 text-center text-muted-foreground">
					<div>
						<FileCheck2 class="mx-auto size-8 opacity-50" />
						<p class="mt-2 text-sm font-medium text-foreground">
							{template.backgroundFileId ? 'พื้นหลังปัจจุบันพร้อมใช้งาน' : 'ยังไม่มี PDF พื้นหลัง'}
						</p>
						<p class="mt-1 text-xs">เลือกไฟล์เพื่อดูตัวอย่างก่อนแนบ</p>
					</div>
				</div>
			{/if}
		</div>
	</div>
</section>
