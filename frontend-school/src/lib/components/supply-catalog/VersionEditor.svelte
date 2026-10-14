<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import {
		appendVersion,
		updateVersion,
		type SupplyVersion,
		type SupplyItemDetail
	} from '#lib/api/supply-catalog.js';
	import { uploadFile, deleteFile } from '#lib/api/files.js';
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
	import { toast } from 'svelte-sonner';
	let {
		item,
		version = null,
		append,
		onsaved,
		oncancel
	}: {
		item: SupplyItemDetail;
		version?: SupplyVersion | null;
		append: boolean;
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
	const staged = new SvelteSet<string>();
	let disposed = false;
	async function cleanup() {
		try {
			for (const id of staged) {
				await deleteFile(id);
				staged.delete(id);
			}
			return true;
		} catch (e) {
			actionError = message(e, 'ล้างรูปชั่วคราวไม่สำเร็จ');
			return false;
		}
	}
	export async function discard() {
		if (busy) return false;
		return cleanup();
	}
	onDestroy(() => {
		disposed = true;
		if (staged.size)
			void cleanup().then((ok) => {
				if (!ok) toast.error('ล้างรูปชั่วคราวไม่สำเร็จ');
			});
	});
	async function upload(file: File | undefined) {
		if (!file || busy || disposed) return;
		uploading = true;
		actionError = null;
		try {
			const normalized = await normalizeCatalogImage(file);
			if (disposed) return;
			const uploaded = await uploadFile(normalized, 'supply_catalog_image');
			staged.add(uploaded.id);
			if (disposed) {
				await cleanup();
				return;
			}
			const old = imageFileId;
			if (old && staged.has(old)) {
				await deleteFile(old);
				staged.delete(old);
			}
			imageFileId = uploaded.id;
		} catch (e) {
			if (!disposed) actionError = message(e, 'อัปโหลดรูปไม่สำเร็จ');
		} finally {
			if (!disposed) uploading = false;
		}
	}
	async function removeImage() {
		if (busy || disposed) return;
		removing = true;
		actionError = null;
		try {
			if (imageFileId && staged.has(imageFileId)) {
				await deleteFile(imageFileId);
				staged.delete(imageFileId);
			}
			imageFileId = null;
		} catch (e) {
			actionError = message(e, 'นำรูปออกไม่สำเร็จ');
		} finally {
			if (!disposed) removing = false;
		}
	}
	async function cancel() {
		if (busy || disposed) return;
		pending = true;
		if ((await cleanup()) && !disposed) oncancel();
		if (!disposed) pending = false;
	}
	async function save() {
		if (busy || disposed) return;
		submitted = true;
		actionError = null;
		if (Object.keys(validatePriceFields({ price, shop })).length) return;
		pending = true;
		try {
			for (const id of staged)
				if (id !== imageFileId) {
					await deleteFile(id);
					staged.delete(id);
				}
			const payload = {
				price,
				shop: trimSupplyText(shop),
				revision: append ? item.revision : (version ?? item.latestVersion).revision
			};
			const saved = append
				? await appendVersion(item.id, { ...payload, imageFileId })
				: await updateVersion(item.id, (version ?? item.latestVersion).id, payload);
			staged.clear();
			if (!disposed) onsaved(saved);
		} catch (e) {
			if (!disposed) actionError = message(e, 'บันทึกราคาไม่สำเร็จ');
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

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
				{#if imageFileId}<CatalogImage
						fileId={imageFileId}
						resourceId={staged.has(imageFileId) ? undefined : item.id}
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
						onchange={(event) => upload(event.currentTarget.files?.[0])}
					/>
				</div>
				{#if uploading || removing}<p role="status" class="text-sm">
						{removing ? 'กำลังนำรูปสินค้าออก' : 'กำลังอัปโหลดรูปสินค้า'}
					</p>{/if}
			</section>{/if}
	</fieldset>
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
