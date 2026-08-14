<script lang="ts">
	import {
		attachCertificateTemplateAsset,
		deleteCertificateTemplateAsset,
		type CertificateTemplateDetail
	} from '$lib/api/certificates';
	import { deleteFile, uploadCertificateTemplateFile, type FileMetadata } from '$lib/api/files';
	import { LoadingButton } from '$lib/components/app-state';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import {
		AlertTriangle,
		FileImage,
		FileType2,
		ImagePlus,
		RefreshCw,
		Trash2,
		Upload
	} from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	type CertificateTemplateAsset = CertificateTemplateDetail['assets'][number];
	type AssetKind = CertificateTemplateAsset['kind'];
	type PendingAsset = {
		metadata: FileMetadata;
		kind: AssetKind;
		displayName: string;
		fontWeight?: number;
		rightsConfirmed: boolean;
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
	const images = $derived(template.assets.filter((asset) => asset.kind === 'image'));
	const fonts = $derived(template.assets.filter((asset) => asset.kind === 'font'));

	let imageFile = $state<File | null>(null);
	let imageDisplayName = $state('');
	let fontFile = $state<File | null>(null);
	let fontDisplayName = $state('');
	let fontWeight = $state('400');
	let rightsConfirmed = $state(false);
	let unattachedFile = $state.raw<PendingAsset | null>(null);
	let attachError = $state<Error | null>(null);
	let uploadingKind = $state<AssetKind | null>(null);
	let cleaning = $state(false);
	let deleteTarget = $state.raw<CertificateTemplateAsset | null>(null);
	let deleting = $state(false);
	let imageInputKey = $state(0);
	let fontInputKey = $state(0);

	function asError(error: unknown, fallback: string): Error {
		return error instanceof Error ? error : new Error(fallback);
	}

	function setUnattachedFile(file: PendingAsset | null) {
		unattachedFile = file;
		onpendingchange(file !== null);
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

	function selectFont(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		fontFile = input.files?.[0] ?? null;
		if (fontFile && !fontDisplayName.trim()) fontDisplayName = filenameWithoutExtension(fontFile);
		attachError = null;
		rightsConfirmed = false;
	}

	function clearForm(kind: AssetKind) {
		if (kind === 'image') {
			imageFile = null;
			imageDisplayName = '';
			imageInputKey += 1;
		} else {
			fontFile = null;
			fontDisplayName = '';
			fontWeight = '400';
			rightsConfirmed = false;
			fontInputKey += 1;
		}
	}

	async function attachPendingAsset() {
		if (!unattachedFile) return;
		const pending = unattachedFile;
		try {
			const updated = await attachCertificateTemplateAsset(templateId, {
				fileId: pending.metadata.id,
				kind: pending.kind,
				displayName: pending.displayName,
				fontWeight: pending.fontWeight,
				rightsConfirmed: pending.rightsConfirmed
			});
			setUnattachedFile(null);
			attachError = null;
			clearForm(pending.kind);
			onpatched(updated);
			toast.success(pending.kind === 'font' ? 'เพิ่มฟอนต์แล้ว' : 'เพิ่มรูปประกอบแล้ว');
		} catch (error) {
			attachError = asError(error, 'แนบไฟล์กับแม่แบบไม่สำเร็จ');
		}
	}

	async function uploadAsset(kind: AssetKind, event: SubmitEvent) {
		event.preventDefault();
		if (uploadingKind || unattachedFile) return;
		const file = kind === 'image' ? imageFile : fontFile;
		const displayName = (kind === 'image' ? imageDisplayName : fontDisplayName)
			.trim()
			.replace(/\s+/g, ' ');
		if (!file || !displayName) {
			attachError = new Error('เลือกไฟล์และระบุชื่อที่ใช้แสดงให้ครบ');
			return;
		}
		if (kind === 'font' && !rightsConfirmed) {
			attachError = new Error('ต้องยืนยันสิทธิ์การใช้งานฟอนต์ก่อนอัปโหลด');
			return;
		}

		uploadingKind = kind;
		onpendingchange(true);
		attachError = null;
		try {
			const purpose = kind === 'image' ? 'certificate_template_image' : 'certificate_template_font';
			const metadata = await uploadCertificateTemplateFile(file, purpose, templateId);
			setUnattachedFile({
				metadata,
				kind,
				displayName,
				fontWeight: kind === 'font' ? Number(fontWeight) : undefined,
				rightsConfirmed: kind === 'font' && rightsConfirmed
			});
			await attachPendingAsset();
		} catch (error) {
			attachError = asError(error, 'อัปโหลดทรัพยากรแม่แบบไม่สำเร็จ');
		} finally {
			uploadingKind = null;
			if (!unattachedFile) onpendingchange(false);
		}
	}

	async function retryAttach() {
		if (!unattachedFile || uploadingKind) return;
		uploadingKind = unattachedFile.kind;
		await attachPendingAsset();
		uploadingKind = null;
	}

	async function deleteTemporaryUpload() {
		if (!unattachedFile || cleaning) return;
		cleaning = true;
		try {
			const kind = unattachedFile.kind;
			await deleteFile(unattachedFile.metadata.id, templateId);
			setUnattachedFile(null);
			attachError = null;
			clearForm(kind);
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
			toast.success(deleteTarget.kind === 'font' ? 'ลบฟอนต์แล้ว' : 'ลบรูปประกอบแล้ว');
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
		<h3 id={`asset-title-${template.id}`} class="font-medium">รูปประกอบและฟอนต์</h3>
		<p class="mt-1 text-xs leading-relaxed text-muted-foreground">
			ไฟล์เหล่านี้เป็น private และใช้ได้เฉพาะแม่แบบนี้ ผู้ใช้ต้องยืนยันสิทธิ์ก่อนเพิ่มฟอนต์
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
					<LoadingButton
						size="sm"
						variant="outline"
						loading={uploadingKind !== null}
						onclick={retryAttach}
					>
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
		<form
			class="space-y-4 rounded-xl border bg-muted/15 p-4"
			onsubmit={(event) => uploadAsset('image', event)}
		>
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
				loading={uploadingKind === 'image'}
				disabled={!template.capabilities.canUpdate || !imageFile || unattachedFile !== null}
			>
				<Upload class="size-4" /> อัปโหลดรูป
			</LoadingButton>
		</form>

		<form
			class="space-y-4 rounded-xl border bg-muted/15 p-4"
			onsubmit={(event) => uploadAsset('font', event)}
		>
			<div class="flex items-center gap-2">
				<span class="grid size-9 place-items-center rounded-lg bg-violet-100 text-violet-700">
					<FileType2 class="size-5" />
				</span>
				<div>
					<h4 class="text-sm font-medium">เพิ่มฟอนต์</h4>
					<p class="text-xs text-muted-foreground">TTF หรือ OTF</p>
				</div>
			</div>
			<div class="space-y-2">
				<Label for={`certificate-font-${template.id}`}>ไฟล์ฟอนต์</Label>
				{#key fontInputKey}
					<Input
						id={`certificate-font-${template.id}`}
						type="file"
						accept=".ttf,.otf"
						onchange={selectFont}
						disabled={!template.capabilities.canUpdate || unattachedFile !== null}
					/>
				{/key}
			</div>
			<div class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_8rem]">
				<div class="space-y-2">
					<Label for={`certificate-font-name-${template.id}`}>ชื่อสำหรับเลือกใน editor</Label>
					<Input
						id={`certificate-font-name-${template.id}`}
						bind:value={fontDisplayName}
						maxlength={200}
						placeholder="เช่น TH Sarabun New"
					/>
				</div>
				<div class="space-y-2">
					<Label for={`certificate-font-weight-${template.id}`}>น้ำหนัก</Label>
					<Select.Root type="single" bind:value={fontWeight}>
						<Select.Trigger id={`certificate-font-weight-${template.id}`} class="w-full">
							{fontWeight}
						</Select.Trigger>
						<Select.Content>
							{#each ['100', '200', '300', '400', '500', '600', '700', '800', '900'] as weight (weight)}
								<Select.Item value={weight}>{weight}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			</div>
			<label
				class="flex cursor-pointer items-start gap-3 rounded-lg border bg-background p-3 text-sm"
			>
				<Checkbox bind:checked={rightsConfirmed} class="mt-0.5" />
				<span>
					<strong class="font-medium">ยืนยันว่ามีสิทธิ์ใช้และฝังฟอนต์นี้ในเกียรติบัตร</strong>
					<span class="mt-0.5 block text-xs leading-relaxed text-muted-foreground">
						ผู้ดูแลต้องตรวจเงื่อนไขลิขสิทธิ์ของไฟล์ก่อนอัปโหลด
					</span>
				</span>
			</label>
			<LoadingButton
				type="submit"
				variant="outline"
				loading={uploadingKind === 'font'}
				disabled={!template.capabilities.canUpdate ||
					!fontFile ||
					!rightsConfirmed ||
					unattachedFile !== null}
			>
				<Upload class="size-4" /> อัปโหลดฟอนต์
			</LoadingButton>
		</form>
	</div>

	<div class="grid gap-4 lg:grid-cols-2">
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

		<div class="space-y-2">
			<div class="flex items-center justify-between">
				<h4 class="text-sm font-medium">ฟอนต์ที่แนบแล้ว</h4>
				<Badge variant="secondary">{fonts.length}</Badge>
			</div>
			{#if fonts.length === 0}
				<p class="rounded-lg border border-dashed p-4 text-center text-xs text-muted-foreground">
					ยังไม่มีฟอนต์เพิ่มเติม · ใช้ Sarabun มาตรฐานได้เสมอ
				</p>
			{:else}
				<div class="space-y-2">
					{#each fonts as asset (asset.id)}
						<div class="flex items-center gap-3 rounded-lg border p-3">
							<FileType2 class="size-5 shrink-0 text-violet-600" />
							<div class="min-w-0 flex-1">
								<p class="truncate text-sm font-medium">{asset.displayName}</p>
								<p class="truncate text-xs text-muted-foreground">
									{asset.fontFamily ?? 'ไม่ทราบ family'} · น้ำหนัก {asset.fontWeight ?? '—'}
								</p>
							</div>
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
				ลบได้เมื่อไม่มีองค์ประกอบในแม่แบบอ้างถึงไฟล์นี้ การลบมีผลกับ editor ของแบบนี้ทันที
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
