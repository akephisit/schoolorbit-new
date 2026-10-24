<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		appendVersion,
		updateVersion,
		type SupplyVersion,
		type SupplyItemDetail
	} from '#lib/api/supply-catalog.js';
	import { uploadFile } from '#lib/api/files.js';
	import { PendingUpload } from '#lib/files/pending-upload.js';
	import PendingFileGuard from '#lib/components/files/PendingFileGuard.svelte';
	import LocalDocumentPreview from '#lib/components/files/LocalDocumentPreview.svelte';
	import { normalizeCatalogImage } from '#lib/utils/catalog-image.js';
	import {
		validatePriceFields,
		trimSupplyText,
		message,
		type PriceFieldErrors
	} from '#lib/supply-catalog/drafts.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Alert, AlertTitle, AlertDescription } from '#lib/components/ui/alert/index.js';
	import FieldError from './FieldError.svelte';
	import CatalogImage from './CatalogImage.svelte';
	let {
		item,
		version = null,
		append,
		preservesDraftNavigation,
		onsaved,
		oncancel
	}: {
		item: SupplyItemDetail;
		version?: SupplyVersion | null;
		append: boolean;
		preservesDraftNavigation?: (destination: URL) => boolean;
		onsaved: (item: SupplyItemDetail) => void;
		oncancel: () => void;
	} = $props();
	let price = $state(untrack(() => (version ?? item.latestVersion).price)),
		shop = $state(untrack(() => (version ?? item.latestVersion).shop)),
		imageFileId = $state<string | null>(untrack(() => item.imageFileId ?? null)),
		pending = $state(false),
		uploading = $state(false),
		actionError = $state<string | null>(null);
	let submitted = $state(false),
		removing = $state(false);
	const busy = $derived(pending || uploading || removing);
	const fieldErrors: PriceFieldErrors = $derived(
		submitted ? validatePriceFields({ price, shop }) : {}
	);
	let localImage = $state<PendingUpload | null>(null);
	let preparation = 0;
	let disposed = false;
	const ownerKey = $derived(`${item.id}:${append}:${version?.id ?? ''}`);
	$effect(() => {
		const owner = ownerKey;
		untrack(() => {
			if (localImage && localImage.scope.owner !== owner) cleanup();
		});
	});
	function cleanup(): boolean {
		preparation++;
		localImage?.dispose();
		localImage = null;
		uploading = false;
		return true;
	}
	export async function discard() {
		return !busy && cleanup();
	}
	onDestroy(() => {
		disposed = true;
		cleanup();
	});
	async function upload(file: File | undefined) {
		if (!file || busy || disposed) return;
		const revision = ++preparation,
			owner = ownerKey;
		uploading = true;
		actionError = null;
		try {
			const normalized = await normalizeCatalogImage(file);
			if (disposed || revision !== preparation || owner !== ownerKey) return;
			localImage?.dispose();
			localImage = new PendingUpload(
				normalized,
				{ purpose: 'supply_catalog_image', owner },
				uploadFile
			);
		} catch (cause) {
			if (!disposed) actionError = message(cause, 'เตรียมรูปไม่สำเร็จ');
		} finally {
			if (!disposed && revision === preparation) uploading = false;
		}
	}
	function removeImage() {
		if (!busy && !disposed) {
			localImage?.dispose();
			localImage = null;
			imageFileId = null;
		}
	}
	async function cancel() {
		if (!busy && !disposed) {
			cleanup();
			oncancel();
		}
	}

	async function save() {
		if (busy || disposed) return;
		submitted = true;
		actionError = null;
		if (Object.keys(validatePriceFields({ price, shop })).length) return;
		pending = true;
		try {
			const owner = ownerKey;
			if (localImage) {
				const image = localImage;
				const id = await image.commit();
				if (disposed || image !== localImage || owner !== ownerKey) return;
				imageFileId = id;
			}
			if (disposed || owner !== ownerKey) return;
			const payload = {
				price,
				shop: trimSupplyText(shop),
				revision: append ? item.revision : (version ?? item.latestVersion).revision
			};
			const saved = append
				? await appendVersion(item.id, { ...payload, imageFileId })
				: await updateVersion(item.id, (version ?? item.latestVersion).id, payload);
			if (disposed || owner !== ownerKey) return;
			cleanup();
			onsaved(saved);
		} catch (e) {
			if (!disposed) actionError = message(e, 'บันทึกราคาไม่สำเร็จ');
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<PendingFileGuard {preservesDraftNavigation} dirty={!!localImage} pending={busy && !!localImage} />
<form
	class="w-full max-w-xl space-y-5"
	novalidate
	aria-busy={busy}
	onsubmit={(event) => {
		event.preventDefault();
		void save();
	}}
>
	<fieldset class="space-y-5" disabled={busy}>
		<section class="space-y-3" aria-labelledby="version-price-title">
			<h3 id="version-price-title" class="text-sm font-semibold">ราคาและร้านค้า</h3>
			<p class="text-sm text-muted-foreground">
				{append
					? 'เพิ่มรุ่นราคาใหม่ โดยเก็บรุ่นเดิมไว้ในประวัติ'
					: `แก้ไขราคาและร้านค้าในรุ่น ${(version ?? item.latestVersion).version} โดยไม่สร้างรุ่นใหม่`}
			</p>
			<div class="grid gap-3 sm:grid-cols-2">
				<div class="space-y-1">
					<Label for="version-price">ราคาต่อหน่วย (บาท) *</Label><Input
						id="version-price"
						bind:value={price}
						inputmode="decimal"
						disabled={busy}
						required
						aria-invalid={!!fieldErrors.price}
						aria-describedby={fieldErrors.price ? 'version-price-error' : undefined}
					/><FieldError id="version-price-error" message={fieldErrors.price} />
				</div>
				<div class="space-y-1">
					<Label for="version-shop">ร้านค้า *</Label><Input
						id="version-shop"
						bind:value={shop}
						disabled={busy}
						required
						aria-invalid={!!fieldErrors.shop}
						aria-describedby={fieldErrors.shop ? 'version-shop-error' : undefined}
					/><FieldError id="version-shop-error" message={fieldErrors.shop} />
				</div>
			</div>
		</section>
		{#if append}<section class="space-y-3 border-t pt-4" aria-labelledby="version-image-title">
				<h3 id="version-image-title" class="text-sm font-semibold">รูปประกอบพัสดุ</h3>
				<p id="version-image-help" class="text-sm text-muted-foreground">
					รูปใหม่จะใช้ร่วมกันทุกเวอร์ชัน และแทนที่รูปเก่าเมื่อบันทึกสำเร็จ
				</p>
				{#if localImage}<LocalDocumentPreview file={localImage.file} /><Button
						type="button"
						variant="outline"
						size="sm"
						disabled={busy}
						onclick={removeImage}>นำรูปสินค้าออก</Button
					>{:else if imageFileId}<CatalogImage
						fileId={imageFileId}
						resourceId={item.id}
						alt={item.name}
						class="h-32 rounded-lg"
					/><Button type="button" variant="outline" size="sm" disabled={busy} onclick={removeImage}
						>นำรูปสินค้าออก</Button
					>{/if}
				<div class="space-y-1">
					<Label for="version-image">รูปสินค้า</Label><Input
						id="version-image"
						type="file"
						accept="image/*,.heic,.heif,.tif,.tiff"
						disabled={busy}
						aria-describedby="version-image-help"
						onchange={(event) => {
							const file = event.currentTarget.files?.[0];
							event.currentTarget.value = '';
							void upload(file);
						}}
					/>
				</div>
			</section>{/if}
	</fieldset>
	{#if append}<p role="status" class="min-h-5 text-sm text-muted-foreground">
			{removing ? 'กำลังนำรูปสินค้าออก' : uploading ? 'กำลังเตรียมรูปสินค้า' : ''}
		</p>{/if}
	{#if actionError}<Alert variant="destructive"
			><AlertTitle>ดำเนินการไม่สำเร็จ</AlertTitle><AlertDescription>{actionError}</AlertDescription
			></Alert
		>{/if}
	<div class="flex flex-wrap justify-end gap-2 border-t pt-4">
		<Button type="button" variant="outline" disabled={busy} onclick={cancel}>ยกเลิก</Button
		><LoadingButton type="submit" loading={pending} disabled={uploading || removing}
			>{append ? 'เพิ่มรุ่นราคา' : 'บันทึกรุ่นราคา'}</LoadingButton
		>
	</div>
</form>
