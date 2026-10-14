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
	import { validateItem, itemDraft, message } from '#lib/supply-catalog/drafts.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
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
		error = $state<string | null>(null);
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
			error = message(e, 'ล้างรูปชั่วคราวไม่สำเร็จ');
			return false;
		}
	}
	export async function discard() {
		if (pending || uploading) return false;
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
		if (!file || pending || uploading) return;
		uploading = true;
		error = null;
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
			if (!disposed) error = message(e, 'อัปโหลดรูปไม่สำเร็จ');
		} finally {
			if (!disposed) uploading = false;
		}
	}
	async function removeImage() {
		if (pending || uploading) return;
		try {
			if (imageFileId && staged.has(imageFileId)) {
				await deleteFile(imageFileId);
				staged.delete(imageFileId);
			}
			imageFileId = null;
		} catch (e) {
			error = message(e, 'นำรูปออกไม่สำเร็จ');
		}
	}
	async function cancel() {
		if (pending || uploading) return;
		pending = true;
		if ((await cleanup()) && !disposed) oncancel();
		if (!disposed) pending = false;
	}
	async function save() {
		if (pending || uploading) return;
		error = validateItem({ ...itemDraft(item), price, shop });
		if (error) return;
		pending = true;
		try {
			for (const id of staged)
				if (id !== imageFileId) {
					await deleteFile(id);
					staged.delete(id);
				}
			const payload = {
				price,
				shop: shop.trim(),
				revision: append ? item.revision : (version ?? item.latestVersion).revision
			};
			const saved = append
				? await appendVersion(item.id, { ...payload, imageFileId })
				: await updateVersion(item.id, (version ?? item.latestVersion).id, payload);
			staged.clear();
			if (!disposed) onsaved(saved);
		} catch (e) {
			if (!disposed) error = message(e, 'บันทึกราคาไม่สำเร็จ');
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<form
	class="space-y-4"
	onsubmit={(e) => {
		e.preventDefault();
		void save();
	}}
>
	<p class="text-sm text-muted-foreground">
		{append
			? 'เพิ่มรุ่นราคาใหม่ โดยเก็บรุ่นเดิมไว้ในประวัติ'
			: `แก้ไขราคาและร้านค้าในรุ่น ${(version ?? item.latestVersion).version} โดยไม่สร้างรุ่นใหม่`}
	</p>
	<div class="space-y-1">
		<Label for="version-price">ราคาต่อหน่วย (บาท) *</Label><Input
			id="version-price"
			bind:value={price}
			inputmode="decimal"
			disabled={pending || uploading}
			required
		/>
	</div>
	<div class="space-y-1">
		<Label for="version-shop">ร้านค้า *</Label><Input
			id="version-shop"
			bind:value={shop}
			maxlength={300}
			disabled={pending || uploading}
			required
		/>
	</div>
	{#if append}<section class="space-y-2 rounded-xl border p-3">
			<Label for="version-image">รูปสินค้า</Label>
			<p class="text-xs text-muted-foreground">
				รูปใหม่จะใช้ร่วมกันทุกเวอร์ชัน และแทนที่รูปเก่าเมื่อบันทึกสำเร็จ
			</p>
			{#if imageFileId}<CatalogImage
					fileId={imageFileId}
					resourceId={staged.has(imageFileId) ? undefined : item.id}
					alt={item.name}
					class="h-32 rounded-lg"
				/><Button
					type="button"
					variant="outline"
					size="sm"
					onclick={removeImage}
					disabled={pending || uploading}>นำรูปสินค้าออก</Button
				>{/if}<Input
				id="version-image"
				type="file"
				accept="image/*,.heic,.heif,.tif,.tiff"
				disabled={pending || uploading}
				onchange={(e) => upload(e.currentTarget.files?.[0])}
			/>{#if uploading}<p role="status" class="text-sm">กำลังอัปโหลดรูปสินค้า</p>{/if}
		</section>{/if}
	{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
	<div class="flex justify-end gap-2">
		<Button type="button" variant="outline" disabled={pending || uploading} onclick={cancel}
			>ยกเลิก</Button
		><LoadingButton type="submit" loading={pending} disabled={uploading}
			>{append ? 'เพิ่มรุ่นราคา' : 'บันทึกรุ่นราคา'}</LoadingButton
		>
	</div>
</form>
