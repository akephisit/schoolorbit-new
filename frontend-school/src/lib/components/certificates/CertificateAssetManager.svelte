<script lang="ts">
	import {
		attachCertificateTemplateAsset,
		deleteCertificateTemplateAsset,
		listCertificateSchoolFonts,
		type CertificateTemplateDetail
	} from '$lib/api/certificates';
	import type { SchoolFontSummary } from '$lib/api/school-fonts';
	import { deleteFile, uploadCertificateTemplateFile, type FileMetadata } from '$lib/api/files';
	import { LoadingButton } from '$lib/components/app-state';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { AlertTriangle, FileImage, ImagePlus, RefreshCw, Trash2, Upload } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import CertificateFontBatchUpload from './CertificateFontBatchUpload.svelte';

	type CertificateTemplateAsset = CertificateTemplateDetail['assets'][number];
	type PendingAsset = {
		metadata: FileMetadata;
		displayName: string;
	};

	let {
		template,
		onpatched,
		onpendingchange
	}: {
		template: CertificateTemplateDetail;
		onpatched: (template: CertificateTemplateDetail) => void;
		onpendingchange: (pending: boolean) => void;
	} = $props();

	const templateId = $derived(template.id);
	const images = $derived(template.assets);

	let imageFile = $state<File | null>(null);
	let imageDisplayName = $state('');
	let unattachedFile = $state.raw<PendingAsset | null>(null);
	let attachError = $state<Error | null>(null);
	let uploadingImage = $state(false);
	let cleaning = $state(false);
	let deleteTarget = $state.raw<CertificateTemplateAsset | null>(null);
	let deleting = $state(false);
	let imageInputKey = $state(0);
	let imagePending = false;
	let fontPending = false;
	let schoolFonts = $state.raw<SchoolFontSummary[]>([]);
	let schoolFontsLoading = $state(true);
	let schoolFontsError = $state('');
	let schoolFontsPatchGeneration = 0;

	onMount(() => {
		const targetTemplateId = templateId;
		const generation = schoolFontsPatchGeneration;
		let active = true;
		schoolFontsLoading = true;
		schoolFontsError = '';
		void listCertificateSchoolFonts(targetTemplateId)
			.then((result) => {
				if (active && generation === schoolFontsPatchGeneration) schoolFonts = result.items;
			})
			.catch((error: unknown) => {
				if (!active || generation !== schoolFontsPatchGeneration) return;
				schoolFontsError =
					error instanceof Error ? error.message : 'โหลดคลังฟอนต์ของโรงเรียนไม่สำเร็จ';
			})
			.finally(() => {
				if (active && generation === schoolFontsPatchGeneration) schoolFontsLoading = false;
			});
		return () => {
			active = false;
		};
	});

	function handleSchoolFontsAttached(items: SchoolFontSummary[]) {
		schoolFontsPatchGeneration += 1;
		schoolFonts = items;
		schoolFontsLoading = false;
		schoolFontsError = '';
	}

	function asError(error: unknown, fallback: string): Error {
		return error instanceof Error ? error : new Error(fallback);
	}

	function reportPending() {
		onpendingchange(imagePending || fontPending);
	}

	function setImagePending(pending: boolean) {
		imagePending = pending;
		reportPending();
	}

	function setFontPending(pending: boolean) {
		fontPending = pending;
		reportPending();
	}

	function setUnattachedFile(file: PendingAsset | null) {
		unattachedFile = file;
		setImagePending(file !== null);
	}

	function filenameWithoutExtension(file: File): string {
		return file.name
			.replace(/\.[^.]+$/, '')
			.trim()
			.slice(0, 200);
	}

	function selectImage(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		imageFile = input.files?.[0] ?? null;
		if (imageFile && !imageDisplayName.trim())
			imageDisplayName = filenameWithoutExtension(imageFile);
		attachError = null;
	}

	function clearImageForm() {
		imageFile = null;
		imageDisplayName = '';
		imageInputKey += 1;
	}

	async function attachPendingAsset() {
		if (!unattachedFile) return;
		const pending = unattachedFile;
		try {
			const updated = await attachCertificateTemplateAsset(templateId, {
				fileId: pending.metadata.id,
				kind: 'image',
				displayName: pending.displayName
			});
			setUnattachedFile(null);
			attachError = null;
			clearImageForm();
			onpatched(updated);
			toast.success('เพิ่มรูปประกอบแล้ว');
		} catch (error) {
			attachError = asError(error, 'แนบไฟล์กับแม่แบบไม่สำเร็จ');
		}
	}

	async function uploadImage(event: SubmitEvent) {
		event.preventDefault();
		if (uploadingImage || unattachedFile) return;
		const file = imageFile;
		const displayName = imageDisplayName.trim().replace(/\s+/g, ' ');
		if (!file || !displayName) {
			attachError = new Error('เลือกไฟล์และระบุชื่อที่ใช้แสดงให้ครบ');
			return;
		}

		uploadingImage = true;
		setImagePending(true);
		attachError = null;
		try {
			const metadata = await uploadCertificateTemplateFile(
				file,
				'certificate_template_image',
				templateId
			);
			setUnattachedFile({
				metadata,
				displayName
			});
			await attachPendingAsset();
		} catch (error) {
			attachError = asError(error, 'อัปโหลดทรัพยากรแม่แบบไม่สำเร็จ');
		} finally {
			uploadingImage = false;
			if (!unattachedFile) setImagePending(false);
		}
	}

	async function retryAttach() {
		if (!unattachedFile || uploadingImage) return;
		uploadingImage = true;
		await attachPendingAsset();
		uploadingImage = false;
	}

	async function deleteTemporaryUpload() {
		if (!unattachedFile || cleaning) return;
		cleaning = true;
		try {
			await deleteFile(unattachedFile.metadata.id, templateId);
			setUnattachedFile(null);
			attachError = null;
			clearImageForm();
			toast.success('ลบไฟล์ชั่วคราวแล้ว');
		} catch (error) {
			attachError = asError(error, 'ลบไฟล์ชั่วคราวไม่สำเร็จ');
		} finally {
			cleaning = false;
		}
	}

	async function removeAsset() {
		if (!deleteTarget || deleting) return;
		deleting = true;
		try {
			const updated = await deleteCertificateTemplateAsset(templateId, deleteTarget.id);
			onpatched(updated);
			toast.success('ลบรูปประกอบแล้ว');
			deleteTarget = null;
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ลบทรัพยากรแม่แบบไม่สำเร็จ');
		} finally {
			deleting = false;
		}
	}
</script>

<section class="space-y-5" aria-labelledby={`asset-title-${template.id}`}>
	<div>
		<h3 id={`asset-title-${template.id}`} class="font-medium">รูปประกอบและคลังฟอนต์</h3>
		<p class="mt-1 text-xs leading-relaxed text-muted-foreground">
			รูปประกอบเป็นไฟล์ private ของแม่แบบนี้ ส่วนฟอนต์ที่อัปโหลดจะใช้ร่วมกันได้ทั้งโรงเรียน
		</p>
	</div>

	{#if attachError}
		<div class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm">
			<div class="flex items-start gap-2 text-destructive">
				<AlertTriangle class="mt-0.5 size-4 shrink-0" />
				<p>{attachError.message}</p>
			</div>
			{#if unattachedFile}
				<p class="mt-2 text-xs text-muted-foreground">
					ไฟล์อัปโหลดสำเร็จแต่ยังไม่ถูกแนบกับแม่แบบ ลองแนบซ้ำหรือลบไฟล์ชั่วคราวนี้ได้
				</p>
				<div class="mt-3 flex flex-wrap gap-2">
					<LoadingButton size="sm" variant="outline" loading={uploadingImage} onclick={retryAttach}>
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

	<div class="grid gap-4 xl:grid-cols-2">
		<form class="space-y-4 rounded-xl border bg-muted/15 p-4" onsubmit={uploadImage}>
			<div class="flex items-center gap-2">
				<span class="grid size-9 place-items-center rounded-lg bg-blue-100 text-blue-700">
					<ImagePlus class="size-5" />
				</span>
				<div>
					<h4 class="text-sm font-medium">เพิ่มรูปประกอบ</h4>
					<p class="text-xs text-muted-foreground">PNG, JPG หรือ WebP</p>
				</div>
			</div>
			<div class="space-y-2">
				<Label for={`certificate-image-${template.id}`}>ไฟล์รูปภาพ</Label>
				{#key imageInputKey}
					<Input
						id={`certificate-image-${template.id}`}
						type="file"
						accept=".png,.jpg,.jpeg,.webp"
						onchange={selectImage}
						disabled={!template.capabilities.canUpdate || unattachedFile !== null}
					/>
				{/key}
			</div>
			<div class="space-y-2">
				<Label for={`certificate-image-name-${template.id}`}>ชื่อสำหรับเลือกใน editor</Label>
				<Input
					id={`certificate-image-name-${template.id}`}
					bind:value={imageDisplayName}
					maxlength={200}
					placeholder="เช่น ตรากลุ่มสาระภาษาไทย"
				/>
			</div>
			<LoadingButton
				type="submit"
				variant="outline"
				loading={uploadingImage}
				disabled={!template.capabilities.canUpdate || !imageFile || unattachedFile !== null}
			>
				<Upload class="size-4" /> อัปโหลดรูป
			</LoadingButton>
		</form>

		<div class="space-y-2">
			{#if template.capabilities.canUpdate}
				<CertificateFontBatchUpload
					templateId={template.id}
					onattached={handleSchoolFontsAttached}
					onpendingchange={setFontPending}
				/>
			{/if}
			<p class="px-1 text-xs text-muted-foreground" aria-live="polite">
				{#if schoolFontsLoading}
					กำลังโหลดคลังฟอนต์…
				{:else if schoolFontsError}
					{schoolFontsError}
				{:else}
					คลังโรงเรียนมีฟอนต์พร้อมใช้ {schoolFonts.length} รูปแบบ
				{/if}
			</p>
		</div>
	</div>

	<div class="space-y-2">
		<div class="flex items-center justify-between">
			<h4 class="text-sm font-medium">รูปที่แนบแล้ว</h4>
			<Badge variant="secondary">{images.length}</Badge>
		</div>
		{#if images.length === 0}
			<p class="rounded-lg border border-dashed p-4 text-center text-xs text-muted-foreground">
				ยังไม่มีรูปประกอบ
			</p>
		{:else}
			<div class="space-y-2">
				{#each images as asset (asset.id)}
					<div class="flex items-center gap-3 rounded-lg border p-3">
						<FileImage class="size-5 shrink-0 text-blue-600" />
						<span class="min-w-0 flex-1 truncate text-sm font-medium">{asset.displayName}</span>
						{#if template.capabilities.canUpdate}
							<Button
								size="icon-sm"
								variant="ghost"
								onclick={() => (deleteTarget = asset)}
								aria-label={`ลบ ${asset.displayName}`}
							>
								<Trash2 class="size-4" />
							</Button>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	</div>
</section>

<AlertDialog.Root
	open={deleteTarget !== null}
	onOpenChange={(open) => !open && (deleteTarget = null)}
>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>ลบ “{deleteTarget?.displayName ?? ''}”?</AlertDialog.Title>
			<AlertDialog.Description>
				ลบได้เมื่อไม่มีองค์ประกอบในแม่แบบอ้างถึงรูปนี้ การลบมีผลกับ editor ของแบบนี้ทันที
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel disabled={deleting}>ยกเลิก</AlertDialog.Cancel>
			<AlertDialog.Action
				onclick={removeAsset}
				disabled={deleting}
				class="bg-destructive text-white"
			>
				{deleting ? 'กำลังลบ...' : 'ลบไฟล์'}
			</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
