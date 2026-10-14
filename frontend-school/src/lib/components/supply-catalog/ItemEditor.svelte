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
		validateItemFields,
		validateSourceUrl,
		trimSupplyText,
		money,
		type ItemFieldErrors,
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
	import { Alert, AlertTitle, AlertDescription } from '#lib/components/ui/alert/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import FieldError from './FieldError.svelte';
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
		if (controlsDisabled || disposed) return;
		const ticket = duplicateRequest.begin();
		checking = true;
		actionError = null;
		try {
			const matches = await findDuplicates(draft.name.trim(), draft.unit.trim(), {
				signal: ticket.signal
			});
			if (duplicateRequest.isCurrent(ticket.revision))
				duplicates = matches.filter((match) => match.id !== item?.id);
		} catch (e) {
			if (duplicateRequest.isCurrent(ticket.revision))
				actionError = message(e, 'ตรวจรายการคล้ายกันไม่สำเร็จ');
		} finally {
			if (duplicateRequest.isCurrent(ticket.revision)) checking = false;
		}
	}
	let pending = $state(false),
		uploading = $state<'image' | 'source' | null>(null),
		actionError = $state<string | null>(null);
	let submitted = $state(false),
		approvalConfirm = $state(false),
		removing = $state<'image' | 'source' | null>(null);
	const busy = $derived(pending || !!uploading || !!removing);
	const controlsDisabled = $derived(busy || approvalConfirm);
	const fieldErrors: ItemFieldErrors & { sourceUrl?: string } = $derived.by(() =>
		submitted ? validationErrors() : {}
	);
	function validationErrors(): ItemFieldErrors & { sourceUrl?: string } {
		const fields = validateItemFields(draft, { metadataOnly: mode === 'edit' });
		const source = mode === 'propose' && !sourceFileId ? validateSourceUrl(sourceUrl) : null;
		return source ? { ...fields, sourceUrl: source } : fields;
	}
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
		if (controlsDisabled || disposed) return;
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
		if (controlsDisabled || disposed) return;
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
			actionError = message(e, 'ล้างไฟล์ชั่วคราวไม่สำเร็จ กรุณาลองอีกครั้ง');
			return false;
		}
	}
	async function cancel() {
		if (controlsDisabled) return;
		pending = true;
		if ((await cleanup()) && !disposed) oncancel();
		if (!disposed) pending = false;
	}
	export async function discard() {
		if (controlsDisabled) return false;
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
		if (!file || controlsDisabled || disposed) return;
		uploading = kind;
		actionError = null;
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
			if (!disposed) actionError = message(e, 'อัปโหลดรูปไม่สำเร็จ');
		} finally {
			if (!disposed) uploading = null;
		}
	}
	async function removeImage() {
		if (controlsDisabled) return;
		const id = draft.imageFileId;
		removing = 'image';
		actionError = null;
		try {
			if (id && staged.has(id)) {
				await deleteFile(id);
				staged.delete(id);
			}
			draft.imageFileId = null;
		} catch (e) {
			actionError = message(e, 'ลบรูปชั่วคราวไม่สำเร็จ');
		} finally {
			if (!disposed) removing = null;
		}
	}
	async function removeSource() {
		if (controlsDisabled) return;
		removing = 'source';
		actionError = null;
		try {
			if (sourceFileId && staged.has(sourceFileId)) {
				await deleteFile(sourceFileId);
				staged.delete(sourceFileId);
			}
			sourceFileId = null;
		} catch (e) {
			actionError = message(e, 'ลบรูปแหล่งที่มาไม่สำเร็จ');
		} finally {
			if (!disposed) removing = null;
		}
	}
	async function requestSave() {
		if (controlsDisabled || disposed) return;
		submitted = true;
		actionError = null;
		if (Object.keys(validationErrors()).length) return;
		if (mode === 'approve') {
			approvalConfirm = true;
			return;
		}
		await save();
	}
	async function save() {
		if (busy || disposed || (mode === 'approve' && !approvalConfirm)) return;
		submitted = true;
		actionError = null;
		if (Object.keys(validationErrors()).length) {
			approvalConfirm = false;
			return;
		}
		duplicateRequest.abort();
		checking = false;
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
									sourceUrl: trimSupplyText(sourceUrl) || null
								})
							: await createItem(payload);
			staged.clear();
			if (!disposed) onsaved(saved);
		} catch (e) {
			if (!disposed)
				actionError = message(
					e,
					'บันทึกไม่สำเร็จ ข้อมูลที่กรอกยังอยู่ กรุณาตรวจสอบก่อนส่งอีกครั้ง'
				);
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
		void requestSave();
	}}
>
	<fieldset class="space-y-5" disabled={controlsDisabled}>
		<section class="space-y-3" aria-labelledby="supply-details-title">
			<h3 id="supply-details-title" class="text-sm font-semibold">ข้อมูลพัสดุ</h3>
			<div class="space-y-1">
				<Label for="supply-name">ชื่อพัสดุ *</Label><Input
					id="supply-name"
					bind:value={draft.name}
					required
					disabled={controlsDisabled}
					aria-invalid={!!fieldErrors.name}
					aria-describedby={fieldErrors.name ? 'supply-name-error' : undefined}
				/><FieldError id="supply-name-error" message={fieldErrors.name} />
			</div>
			<div class="grid gap-3 sm:grid-cols-2">
				<div class="space-y-1">
					<Label for="supply-category">หมวดหมู่ *</Label><Select.Root
						type="single"
						bind:value={draft.categoryId}
						disabled={controlsDisabled}
						><Select.Trigger
							id="supply-category"
							class="w-full"
							aria-required="true"
							aria-invalid={!!fieldErrors.categoryId}
							aria-describedby={fieldErrors.categoryId ? 'supply-category-error' : undefined}
							>{categories.find((category) => category.id === draft.categoryId)?.name ??
								'เลือกหมวดหมู่'}</Select.Trigger
						><Select.Content
							>{#each categories as category (category.id)}<Select.Item value={category.id}
									>{category.name}</Select.Item
								>{/each}</Select.Content
						></Select.Root
					><FieldError id="supply-category-error" message={fieldErrors.categoryId} />
				</div>
				<div class="space-y-1">
					<Label for="supply-unit">หน่วยนับ *</Label><Input
						id="supply-unit"
						bind:value={draft.unit}
						required
						disabled={controlsDisabled}
						placeholder="เช่น อัน กล่อง รีม"
						aria-invalid={!!fieldErrors.unit}
						aria-describedby={fieldErrors.unit ? 'supply-unit-error' : undefined}
					/><FieldError id="supply-unit-error" message={fieldErrors.unit} />
				</div>
			</div>
			<div class="space-y-1">
				<Label for="supply-description">รายละเอียด</Label><Textarea
					id="supply-description"
					bind:value={draft.description}
					rows={3}
					disabled={controlsDisabled}
					aria-invalid={!!fieldErrors.description}
					aria-describedby={fieldErrors.description ? 'supply-description-error' : undefined}
				/><FieldError id="supply-description-error" message={fieldErrors.description} />
			</div>
			<div class="space-y-2">
				<Button
					type="button"
					variant="outline"
					size="sm"
					disabled={controlsDisabled || !draft.name.trim() || !draft.unit.trim() || checking}
					onclick={checkDuplicates}>{checking ? 'กำลังตรวจรายการ' : 'ตรวจรายการคล้ายกัน'}</Button
				>{#if duplicates}<div class="space-y-2 rounded-xl border p-3 text-sm">
						{#if duplicates.length === 0}<p class="text-muted-foreground">
								ไม่พบรายการคล้ายกันในคลัง
							</p>{:else}<p class="font-medium">พบพัสดุที่อาจเป็นรายการเดียวกัน</p>
							<ul class="space-y-2">
								{#each duplicates as match (match.id)}<li
										class="flex flex-wrap items-center justify-between gap-2"
									>
										<span class="min-w-0 break-words">{match.name} · {match.unit}</span
										>{#if mode === 'approve'}<Button
												type="button"
												size="sm"
												variant="outline"
												disabled={controlsDisabled}
												onclick={() => selectTarget(match)}>รับเพิ่มรุ่น</Button
											>{/if}
									</li>{/each}
							</ul>{/if}
					</div>{/if}
			</div>
		</section>
		{#if mode !== 'edit'}<section
				class="space-y-3 border-t pt-4"
				aria-labelledby="supply-price-title"
			>
				<h3 id="supply-price-title" class="text-sm font-semibold">ราคาและร้านค้า</h3>
				<div class="grid gap-3 sm:grid-cols-2">
					<div class="space-y-1">
						<Label for="supply-price">ราคาต่อหน่วย (บาท) *</Label><Input
							id="supply-price"
							inputmode="decimal"
							bind:value={draft.price}
							required
							disabled={controlsDisabled}
							aria-invalid={!!fieldErrors.price}
							aria-describedby={fieldErrors.price ? 'supply-price-error' : undefined}
						/><FieldError id="supply-price-error" message={fieldErrors.price} />
					</div>
					<div class="space-y-1">
						<Label for="supply-shop">ร้านค้า *</Label><Input
							id="supply-shop"
							bind:value={draft.shop}
							required
							disabled={controlsDisabled}
							aria-invalid={!!fieldErrors.shop}
							aria-describedby={fieldErrors.shop ? 'supply-shop-error' : undefined}
						/><FieldError id="supply-shop-error" message={fieldErrors.shop} />
					</div>
				</div>
			</section>{/if}
		<section class="space-y-3 border-t pt-4" aria-labelledby="supply-image-title">
			<h3 id="supply-image-title" class="text-sm font-semibold">รูปประกอบพัสดุ</h3>
			<p id="supply-image-help" class="text-sm text-muted-foreground">
				ใช้รูปปัจจุบันร่วมกันทุกเวอร์ชัน เมื่อเปลี่ยนรูป ระบบจะลบรูปเก่า
			</p>
			{#if draft.imageFileId}<CatalogImage
					fileId={draft.imageFileId}
					resourceId={imageResource}
					alt={draft.name || 'รูปสินค้า'}
					class="h-32 rounded-lg"
				/><Button
					type="button"
					size="sm"
					variant="outline"
					disabled={controlsDisabled}
					onclick={removeImage}>นำรูปสินค้าออก</Button
				>{/if}
			<div class="space-y-1">
				<Label for="supply-image">รูปสินค้า</Label><Input
					id="supply-image"
					type="file"
					accept="image/*,.heic,.heif,.tif,.tiff"
					disabled={controlsDisabled}
					aria-describedby="supply-image-help"
					onchange={(event) => upload(event.currentTarget.files?.[0], 'image')}
				/>
			</div>
			{#if uploading === 'image' || removing === 'image'}<p role="status" class="text-sm">
					{removing === 'image' ? 'กำลังนำรูปสินค้าออก' : 'กำลังอัปโหลดรูปสินค้า'}
				</p>{/if}
		</section>
		{#if mode === 'propose'}<section
				class="space-y-3 border-t pt-4"
				aria-labelledby="supply-source-title"
			>
				<h3 id="supply-source-title" class="text-sm font-semibold">แหล่งที่มา (ไม่บังคับ)</h3>
				<p id="supply-source-help" class="text-sm text-muted-foreground">
					ลิงก์หรือรูปแหล่งที่มาใช้ระหว่างตรวจข้อเสนอ และจะถูกลบหลังอนุมัติ
				</p>
				<div class="space-y-1">
					<Label for="supply-source-url">ลิงก์แหล่งที่มา</Label><Input
						id="supply-source-url"
						type="url"
						bind:value={sourceUrl}
						disabled={controlsDisabled || !!sourceFileId}
						placeholder="https://"
						aria-invalid={!!fieldErrors.sourceUrl}
						aria-describedby={fieldErrors.sourceUrl
							? 'supply-source-help supply-source-url-error'
							: 'supply-source-help'}
					/><FieldError id="supply-source-url-error" message={fieldErrors.sourceUrl} />
				</div>
				<div class="space-y-1">
					<Label for="supply-source-image">รูปแหล่งที่มา</Label><Input
						id="supply-source-image"
						type="file"
						accept="image/*,.heic,.heif,.tif,.tiff"
						disabled={controlsDisabled}
						aria-describedby="supply-source-help"
						onchange={(event) => upload(event.currentTarget.files?.[0], 'source')}
					/>
				</div>
				{#if sourceFileId}<CatalogImage
						fileId={sourceFileId}
						alt="รูปแหล่งที่มา"
						class="h-32 rounded-lg"
					/><Button
						type="button"
						variant="outline"
						size="sm"
						disabled={controlsDisabled}
						onclick={removeSource}>นำรูปแหล่งที่มาออก</Button
					>{/if}{#if uploading === 'source' || removing === 'source'}<p
						role="status"
						class="text-sm"
					>
						{removing === 'source' ? 'กำลังนำรูปแหล่งที่มาออก' : 'กำลังอัปโหลดรูปแหล่งที่มา'}
					</p>{/if}
			</section>{/if}
		{#if mode === 'approve'}<section
				class="space-y-3 border-t pt-4"
				aria-labelledby="supply-approval-title"
			>
				<h3 id="supply-approval-title" class="text-sm font-semibold">รับข้อเสนอเข้าคลัง</h3>
				<p class="text-sm text-muted-foreground">
					แก้ไขทุกช่องก่อนรับเข้าคลัง หรือเลือกพัสดุเดิมเพื่อเพิ่มรุ่นราคา
				</p>
				{#if target}<div
						class="flex flex-wrap items-center justify-between gap-2 rounded-xl border p-3"
					>
						<span class="min-w-0 break-words text-sm">เพิ่มรุ่นใหม่ให้ {target.name}</span><Button
							type="button"
							variant="ghost"
							size="sm"
							disabled={controlsDisabled}
							onclick={resetTarget}>เปลี่ยนเป็นพัสดุใหม่</Button
						>
					</div>{:else}<p class="text-sm">รับเป็นพัสดุใหม่</p>{/if}<ItemPicker
					onselect={selectTarget}
					disabled={controlsDisabled}
				/>
			</section>{/if}
	</fieldset>
	{#if actionError}<Alert variant="destructive"
			><AlertTitle>ดำเนินการไม่สำเร็จ</AlertTitle><AlertDescription>{actionError}</AlertDescription
			></Alert
		>{/if}
	<div class="flex flex-wrap justify-end gap-2 border-t pt-4">
		<Button type="button" variant="outline" disabled={controlsDisabled} onclick={cancel}
			>ยกเลิก</Button
		><LoadingButton
			type="submit"
			loading={pending}
			disabled={!!uploading || !!removing || approvalConfirm || categories.length === 0}
			>{mode === 'approve'
				? 'อนุมัติและรับเข้าคลัง'
				: mode === 'propose'
					? 'ส่งข้อเสนอ'
					: 'บันทึกพัสดุ'}</LoadingButton
		>
	</div>
</form>

<AlertDialog.Root
	open={approvalConfirm}
	onOpenChange={(open) => {
		if (!busy) approvalConfirm = open;
	}}
	><AlertDialog.Content
		escapeKeydownBehavior={busy ? 'ignore' : 'close'}
		interactOutsideBehavior="ignore"
		><AlertDialog.Header
			><AlertDialog.Title>ยืนยันรับข้อเสนอเข้าคลัง?</AlertDialog.Title><AlertDialog.Description
				>เมื่ออนุมัติ ระบบจะลบรูปหรือลิงก์แหล่งที่มาชั่วคราวถาวร</AlertDialog.Description
			></AlertDialog.Header
		>
		<dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2 text-sm">
			<dt class="text-muted-foreground">พัสดุ</dt>
			<dd class="break-words font-medium">{draft.name}</dd>
			<dt class="text-muted-foreground">ราคาต่อหน่วย</dt>
			<dd class="break-words">{approvalConfirm ? money(draft.price) : ''} ฿ / {draft.unit}</dd>
			<dt class="text-muted-foreground">ร้านค้า</dt>
			<dd class="break-words">{draft.shop}</dd>
		</dl>
		{#if actionError}<Alert variant="destructive"
				><AlertTitle>อนุมัติไม่สำเร็จ</AlertTitle><AlertDescription>{actionError}</AlertDescription
				></Alert
			>{/if}<AlertDialog.Footer
			><Button
				type="button"
				variant="outline"
				disabled={busy}
				onclick={() => (approvalConfirm = false)}>ยกเลิก</Button
			><LoadingButton
				type="button"
				loading={pending}
				disabled={!!uploading || !!removing}
				onclick={save}>ยืนยันอนุมัติ</LoadingButton
			></AlertDialog.Footer
		></AlertDialog.Content
	></AlertDialog.Root
>
