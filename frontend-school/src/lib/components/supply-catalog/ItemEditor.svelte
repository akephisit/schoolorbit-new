<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import {
		createItem,
		updateItem,
		createProposal,
		approveProposal,
		findDuplicates,
		type SupplyItem,
		type SupplyCategory,
		type SupplyProposal
	} from '#lib/api/supply-catalog.js';
	import { uploadFile, deleteFile } from '#lib/api/files.js';
	import { normalizeCatalogImage } from '#lib/utils/catalog-image.js';
	import {
		emptyItem,
		itemDraft,
		validateItem,
		cleanItem,
		message,
		type ItemDraft
	} from '#lib/supply-catalog/drafts.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import CatalogImage from './CatalogImage.svelte';
	import ItemPicker from './ItemPicker.svelte';
	import { toast } from 'svelte-sonner';
	let {
		mode,
		item = null,
		proposal = null,
		categories,
		onsaved,
		oncancel
	}: {
		mode: 'create' | 'edit' | 'propose' | 'approve';
		item?: SupplyItem | null;
		proposal?: SupplyProposal | null;
		categories: SupplyCategory[];
		onsaved: (item: SupplyItem | SupplyProposal) => void;
		oncancel: () => void;
	} = $props();
	let draft = $state<ItemDraft>(
		untrack(() =>
			item ? itemDraft(item) : proposal ? itemDraft(proposal) : emptyItem(categories[0]?.id)
		)
	);
	let sourceUrl = $state(''),
		sourceFileId = $state<string | null>(null);
	let target = $state.raw<SupplyItem | null>(null);
	const duplicateRequest = new LatestRequest();
	let duplicates = $state.raw<SupplyItem[] | null>(null),
		checking = $state(false);
	let duplicateContext = '';
	$effect(() => {
		const context = `${draft.name}|${draft.unit}`;
		if (context === duplicateContext) return;
		duplicateContext = context;
		duplicateRequest.abort();
		duplicates = null;
		checking = false;
	});
	async function checkDuplicates() {
		const ticket = duplicateRequest.begin();
		checking = true;
		error = null;
		try {
			const matches = await findDuplicates(draft.name.trim(), draft.unit.trim(), {
				signal: ticket.signal
			});
			if (duplicateRequest.isCurrent(ticket.revision))
				duplicates = matches.filter((match) => match.id !== item?.id);
		} catch (e) {
			if (duplicateRequest.isCurrent(ticket.revision))
				error = message(e, 'ตรวจรายการคล้ายกันไม่สำเร็จ');
		} finally {
			if (duplicateRequest.isCurrent(ticket.revision)) checking = false;
		}
	}
	let pending = $state(false),
		uploading = $state<'image' | 'source' | null>(null),
		error = $state<string | null>(null);
	const staged = new SvelteSet<string>();
	let disposed = false;
	const imageResource = $derived(
		staged.has(draft.imageFileId ?? '')
			? undefined
			: target?.imageFileId === draft.imageFileId
				? target?.id
				: (item?.id ?? proposal?.id)
	);
	function selectTarget(selected: SupplyItem) {
		if (target?.imageFileId === draft.imageFileId)
			draft.imageFileId = proposal?.imageFileId ?? null;
		target = selected;
		draft.name = selected.name;
		draft.description = selected.description;
		draft.categoryId = selected.categoryId;
		draft.unit = selected.unit;
		if (!draft.imageFileId) draft.imageFileId = selected.imageFileId ?? null;
	}
	function resetTarget() {
		if (target?.imageFileId === draft.imageFileId)
			draft.imageFileId = proposal?.imageFileId ?? null;
		target = null;
	}
	async function cleanup(): Promise<boolean> {
		try {
			for (const id of staged) {
				await deleteFile(id);
				staged.delete(id);
			}
			return true;
		} catch (e) {
			error = message(e, 'ล้างไฟล์ชั่วคราวไม่สำเร็จ กรุณาลองอีกครั้ง');
			return false;
		}
	}
	async function cancel() {
		if (pending || uploading) return;
		pending = true;
		if ((await cleanup()) && !disposed) oncancel();
		if (!disposed) pending = false;
	}
	export async function discard() {
		if (pending || uploading) return false;
		return cleanup();
	}
	onDestroy(() => {
		disposed = true;
		duplicateRequest.abort();
		if (staged.size)
			void cleanup().then((ok) => {
				if (!ok) toast.error('ล้างไฟล์ชั่วคราวไม่สำเร็จ');
			});
	});
	async function upload(file: File | undefined, kind: 'image' | 'source') {
		if (!file || pending || uploading) return;
		uploading = kind;
		error = null;
		try {
			const normalized = await normalizeCatalogImage(file);
			if (disposed) return;
			const uploaded = await uploadFile(
				normalized,
				kind === 'image' ? 'supply_catalog_image' : 'supply_proposal_source'
			);
			staged.add(uploaded.id);
			if (disposed) {
				await cleanup();
				return;
			}
			const old = kind === 'image' ? draft.imageFileId : sourceFileId;
			if (old && staged.has(old)) {
				await deleteFile(old);
				staged.delete(old);
			}
			if (kind === 'image') draft.imageFileId = uploaded.id;
			else {
				sourceFileId = uploaded.id;
				sourceUrl = '';
			}
		} catch (e) {
			if (!disposed) error = message(e, 'อัปโหลดรูปไม่สำเร็จ');
		} finally {
			if (!disposed) uploading = null;
		}
	}
	async function removeImage() {
		if (pending || uploading) return;
		const id = draft.imageFileId;
		try {
			if (id && staged.has(id)) {
				await deleteFile(id);
				staged.delete(id);
			}
			draft.imageFileId = null;
		} catch (e) {
			error = message(e, 'ลบรูปชั่วคราวไม่สำเร็จ');
		}
	}
	async function removeSource() {
		if (pending || uploading) return;
		try {
			if (sourceFileId && staged.has(sourceFileId)) {
				await deleteFile(sourceFileId);
				staged.delete(sourceFileId);
			}
			sourceFileId = null;
		} catch (e) {
			error = message(e, 'ลบรูปแหล่งที่มาไม่สำเร็จ');
		}
	}
	async function save() {
		if (pending || uploading || disposed) return;
		error = validateItem(draft);
		if (error) return;
		if (sourceUrl.trim()) {
			try {
				const url = new URL(sourceUrl);
				if (
					!['https:', 'http:'].includes(url.protocol) ||
					!url.hostname ||
					!!url.username ||
					!!url.password
				)
					throw new Error();
			} catch {
				error = 'กรอกลิงก์แหล่งที่มาเป็น http หรือ https';
				return;
			}
		}
		pending = true;
		try {
			for (const id of staged)
				if (id !== draft.imageFileId && id !== sourceFileId) {
					await deleteFile(id);
					staged.delete(id);
				}
			const payload = cleanItem(draft);
			const saved =
				mode === 'edit' && item
					? await updateItem(item.id, {
							name: payload.name,
							description: payload.description,
							categoryId: payload.categoryId,
							unit: payload.unit,
							imageFileId: payload.imageFileId,
							revision: item.revision
						})
					: mode === 'approve' && proposal
						? await approveProposal(proposal.id, {
								...payload,
								revision: proposal.revision,
								existingItemId: target?.id ?? null,
								existingItemRevision: target?.revision ?? null
							})
						: mode === 'propose'
							? await createProposal({
									...payload,
									sourceFileId,
									sourceUrl: sourceUrl.trim() || null
								})
							: await createItem(payload);
			staged.clear();
			if (!disposed) onsaved(saved);
		} catch (e) {
			if (!disposed)
				error = message(e, 'บันทึกไม่สำเร็จ ข้อมูลที่กรอกยังอยู่ กรุณาตรวจสอบก่อนส่งอีกครั้ง');
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
	<fieldset class="space-y-4" disabled={pending || !!uploading}>
		<div class="space-y-1">
			<Label for="supply-name">ชื่อพัสดุ *</Label><Input
				id="supply-name"
				bind:value={draft.name}
				maxlength={200}
				required
			/>
		</div>
		<div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
			<div class="space-y-1">
				<Label for="supply-category">หมวดหมู่ *</Label><Select.Root
					type="single"
					bind:value={draft.categoryId}
					disabled={pending || !!uploading}
					><Select.Trigger id="supply-category" class="w-full"
						>{categories.find((category) => category.id === draft.categoryId)?.name ??
							'เลือกหมวดหมู่'}</Select.Trigger
					><Select.Content
						>{#each categories as category (category.id)}<Select.Item value={category.id}
								>{category.name}</Select.Item
							>{/each}</Select.Content
					></Select.Root
				>
			</div>
			<div class="space-y-1">
				<Label for="supply-unit">หน่วยนับ *</Label><Input
					id="supply-unit"
					bind:value={draft.unit}
					maxlength={80}
					required
					placeholder="เช่น อัน กล่อง รีม"
				/>
			</div>
		</div>
		<div class="space-y-2">
			<Button
				type="button"
				variant="outline"
				size="sm"
				disabled={!draft.name.trim() || !draft.unit.trim() || checking}
				onclick={checkDuplicates}>{checking ? 'กำลังตรวจรายการ' : 'ตรวจรายการคล้ายกัน'}</Button
			>{#if duplicates}<div class="rounded-xl border p-3 text-sm">
					{#if duplicates.length === 0}<p class="text-muted-foreground">
							ไม่พบรายการคล้ายกันในคลัง
						</p>{:else}<p class="font-medium">พบพัสดุที่อาจเป็นรายการเดียวกัน</p>
						<ul class="mt-2 space-y-2">
							{#each duplicates as match (match.id)}<li
									class="flex items-center justify-between gap-2"
								>
									<span class="min-w-0 break-words">{match.name} · {match.unit}</span
									>{#if mode === 'approve'}<Button
											type="button"
											size="sm"
											variant="outline"
											onclick={() => selectTarget(match)}>รับเพิ่มรุ่น</Button
										>{/if}
								</li>{/each}
						</ul>{/if}
				</div>{/if}
		</div>
		<div class="space-y-1">
			<Label for="supply-description">รายละเอียด</Label><Textarea
				id="supply-description"
				bind:value={draft.description}
				maxlength={5000}
				rows={3}
			/>
		</div>
		{#if mode !== 'edit'}<div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
				<div class="space-y-1">
					<Label for="supply-price">ราคาต่อหน่วย (บาท) *</Label><Input
						id="supply-price"
						inputmode="decimal"
						bind:value={draft.price}
						required
					/>
				</div>
				<div class="space-y-1">
					<Label for="supply-shop">ร้านค้า *</Label><Input
						id="supply-shop"
						bind:value={draft.shop}
						maxlength={300}
						required
					/>
				</div>
			</div>{/if}
		<section class="space-y-2 rounded-xl border p-3">
			<Label for="supply-image">รูปสินค้า</Label>
			<p class="text-xs text-muted-foreground">
				ใช้รูปปัจจุบันร่วมกันทุกเวอร์ชัน เมื่อเปลี่ยนรูป ระบบจะลบรูปเก่า
			</p>
			{#if draft.imageFileId}<CatalogImage
					fileId={draft.imageFileId}
					resourceId={imageResource}
					alt={draft.name || 'รูปสินค้า'}
					class="h-32 rounded-lg"
				/><Button type="button" size="sm" variant="outline" onclick={removeImage}
					>นำรูปสินค้าออก</Button
				>{/if}<Input
				id="supply-image"
				type="file"
				accept="image/*,.heic,.heif,.tif,.tiff"
				onchange={(e) => upload(e.currentTarget.files?.[0], 'image')}
			/>{#if uploading === 'image'}<p role="status" class="text-sm">กำลังอัปโหลดรูปสินค้า</p>{/if}
		</section>
		{#if mode === 'propose'}<section class="space-y-3 rounded-xl border p-3">
				<h3 class="text-sm font-medium">แหล่งที่มา (ไม่บังคับ)</h3>
				<p class="text-xs text-muted-foreground">
					ลิงก์หรือรูปแหล่งที่มาใช้ระหว่างตรวจข้อเสนอ และจะถูกลบหลังอนุมัติ
				</p>
				<div class="space-y-1">
					<Label for="supply-source-url">ลิงก์แหล่งที่มา</Label><Input
						id="supply-source-url"
						type="url"
						bind:value={sourceUrl}
						disabled={!!sourceFileId}
						placeholder="https://"
						maxlength={2048}
					/>
				</div>
				<div class="space-y-1">
					<Label for="supply-source-image">รูปแหล่งที่มา</Label><Input
						id="supply-source-image"
						type="file"
						accept="image/*,.heic,.heif,.tif,.tiff"
						onchange={(e) => upload(e.currentTarget.files?.[0], 'source')}
					/>{#if sourceFileId}<CatalogImage
							fileId={sourceFileId}
							alt="รูปแหล่งที่มา"
							class="h-32 rounded-lg"
						/><Button type="button" variant="outline" size="sm" onclick={removeSource}
							>นำรูปแหล่งที่มาออก</Button
						>{/if}{#if uploading === 'source'}<p role="status" class="text-sm">
							กำลังอัปโหลดรูปแหล่งที่มา
						</p>{/if}
				</div>
			</section>{/if}
		{#if mode === 'approve'}<section class="space-y-3">
				<h3 class="text-sm font-medium">ผลการอนุมัติ</h3>
				<p class="text-xs text-muted-foreground">
					แก้ไขทุกช่องก่อนรับเข้าคลัง หรือเลือกพัสดุเดิมเพื่อเพิ่มรุ่นราคา
				</p>
				{#if target}<div class="flex items-center justify-between gap-2 rounded-xl border p-3">
						<span class="text-sm">เพิ่มรุ่นใหม่ให้ {target.name}</span><Button
							type="button"
							variant="ghost"
							size="sm"
							onclick={resetTarget}>เปลี่ยนเป็นพัสดุใหม่</Button
						>
					</div>{:else}<p class="text-sm">รับเป็นพัสดุใหม่</p>{/if}<ItemPicker
					onselect={selectTarget}
					disabled={pending}
				/>
			</section>{/if}
	</fieldset>
	{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
	<div class="flex justify-end gap-2">
		<Button type="button" variant="outline" disabled={pending || !!uploading} onclick={cancel}
			>ยกเลิก</Button
		><LoadingButton
			type="submit"
			loading={pending}
			disabled={!!uploading || categories.length === 0}
			>{mode === 'approve'
				? 'อนุมัติและรับเข้าคลัง'
				: mode === 'propose'
					? 'ส่งข้อเสนอ'
					: 'บันทึกพัสดุ'}</LoadingButton
		>
	</div>
</form>
