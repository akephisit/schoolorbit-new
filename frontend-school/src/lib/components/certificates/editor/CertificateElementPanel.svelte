<script lang="ts">
	import type { CertificateTemplateDetail } from '$lib/api/certificates';
	import type { SchoolFontSummary } from '$lib/api/school-fonts';
	import {
		imageAssetAspectRatio,
		resetImageAspectRatio,
		setImageAspectRatioLock,
		type CertificateElement,
		type PagePointSize,
		type TextCertificateElement
	} from '$lib/certificates/editor-state';
	import {
		certificateFontVariants,
		findCertificateFontVariant,
		fontVariantPatch,
		selectFontFamily,
		selectFontWeight,
		toggleBoldVariant,
		toggleItalicVariant,
		type CertificateFontVariant
	} from '$lib/certificates/font-variants';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Textarea } from '$lib/components/ui/textarea';
	import {
		AlignCenter,
		AlignLeft,
		AlignRight,
		Braces,
		Copy,
		ImagePlus,
		QrCode,
		Trash2,
		Type
	} from 'lucide-svelte';
	import CertificateVariablePicker from './CertificateVariablePicker.svelte';

	type TemplateAsset = CertificateTemplateDetail['assets'][number];

	let {
		selectedElement,
		assets,
		schoolFonts,
		pageSize,
		variables,
		hasQr,
		disabled = false,
		onaddtext,
		onaddqr,
		onaddimage,
		onpatch,
		onduplicate,
		ondelete
	}: {
		selectedElement: CertificateElement | null;
		assets: TemplateAsset[];
		schoolFonts: SchoolFontSummary[];
		pageSize: PagePointSize;
		variables: string[];
		hasQr: boolean;
		disabled?: boolean;
		onaddtext: () => void;
		onaddqr: () => void;
		onaddimage: (assetId: string) => void;
		onpatch: (element: CertificateElement) => void;
		onduplicate: () => void;
		ondelete: () => void;
	} = $props();

	let imageAssetId = $state('');
	const imageAssets = $derived(assets.filter((asset) => asset.kind === 'image'));
	const fontVariants = $derived(certificateFontVariants(schoolFonts));
	const fontFamilies = $derived.by(() => {
		const families: CertificateFontVariant[] = [];
		for (const variant of fontVariants) {
			if (!families.some((family) => family.familyKey === variant.familyKey)) {
				families.push(variant);
			}
		}
		return families;
	});
	const currentFontVariant = $derived.by(() =>
		selectedElement?.type === 'text'
			? findCertificateFontVariant(fontVariants, selectedElement)
			: null
	);
	const currentFontWeights = $derived.by(() => {
		if (!currentFontVariant) return [];
		return Array.from(
			new Set(
				fontVariants
					.filter(
						(variant) =>
							variant.familyKey === currentFontVariant.familyKey &&
							variant.style === currentFontVariant.style
					)
					.map((variant) => variant.weight)
			)
		).sort((left, right) => left - right);
	});
	const boldVariant = $derived(toggleBoldVariant(fontVariants, currentFontVariant));
	const italicVariant = $derived(toggleItalicVariant(fontVariants, currentFontVariant));

	function finiteInput(event: Event, fallback: number): number {
		const value = Number((event.currentTarget as HTMLInputElement).value);
		return Number.isFinite(value) ? value : fallback;
	}

	function patchFrame(key: 'x' | 'y' | 'width' | 'height', value: number) {
		if (!selectedElement) return;
		const next = key === 'width' || key === 'height' ? Math.max(12, value) : Math.max(0, value);
		if (
			selectedElement.type === 'image' &&
			selectedElement.lockAspectRatio &&
			(key === 'width' || key === 'height')
		) {
			const centerX = selectedElement.frame.x + selectedElement.frame.width / 2;
			const centerY = selectedElement.frame.y + selectedElement.frame.height / 2;
			const width = key === 'width' ? next : next * selectedElement.aspectRatio;
			const height = width / selectedElement.aspectRatio;
			onpatch({
				...selectedElement,
				frame: {
					x: centerX - width / 2,
					y: centerY - height / 2,
					width,
					height
				}
			});
			return;
		}
		onpatch({
			...selectedElement,
			frame: { ...selectedElement.frame, [key]: next }
		} as CertificateElement);
	}

	function patchRotation(value: number) {
		if (!selectedElement) return;
		onpatch({ ...selectedElement, rotation: ((value % 360) + 360) % 360 } as CertificateElement);
	}

	function patchText(patch: Partial<TextCertificateElement>) {
		if (selectedElement?.type !== 'text') return;
		onpatch({ ...selectedElement, ...patch });
	}

	function applyFontVariant(variant: CertificateFontVariant | null) {
		if (!variant) return;
		patchText(fontVariantPatch(variant));
	}

	function changeFontFamily(familyKey: string) {
		applyFontVariant(selectFontFamily(fontVariants, familyKey));
	}

	function changeFontWeight(weight: string) {
		applyFontVariant(selectFontWeight(fontVariants, currentFontVariant, Number(weight)));
	}

	function toggleShadow(enabled: boolean) {
		patchText({
			shadow: enabled
				? (selectedElement?.type === 'text' && selectedElement.shadow) || {
						offsetX: 1.5,
						offsetY: 1.5,
						blur: 2,
						color: '#00000055'
					}
				: null
		});
	}

	function addSelectedImage() {
		if (!imageAssetId) return;
		onaddimage(imageAssetId);
	}

	function currentImageAsset() {
		if (selectedElement?.type !== 'image') return null;
		return imageAssets.find((asset) => asset.id === selectedElement.assetId) ?? null;
	}

	function changeSelectedImage(assetId: string) {
		if (selectedElement?.type !== 'image') return;
		const asset = imageAssets.find((candidate) => candidate.id === assetId);
		if (!asset) return;
		onpatch(
			resetImageAspectRatio({ ...selectedElement, assetId }, imageAssetAspectRatio(asset), pageSize)
		);
	}

	function changeImageAspectLock(locked: boolean) {
		if (selectedElement?.type !== 'image') return;
		onpatch(setImageAspectRatioLock(selectedElement, locked, pageSize));
	}

	function resetSelectedImageRatio() {
		if (selectedElement?.type !== 'image') return;
		const asset = currentImageAsset();
		if (!asset) return;
		onpatch(resetImageAspectRatio(selectedElement, imageAssetAspectRatio(asset), pageSize));
	}
</script>

<section class="space-y-5" aria-labelledby="certificate-elements-title">
	<div>
		<h2 id="certificate-elements-title" class="text-sm font-semibold">องค์ประกอบ</h2>
		<p class="mt-0.5 text-[0.7rem] leading-relaxed text-muted-foreground">
			ข้อความเป็น plain text และแทรกตัวแปรจากคอลัมน์รายชื่อได้
		</p>
	</div>

	<div class="grid grid-cols-2 gap-2">
		<Button type="button" size="sm" variant="outline" {disabled} onclick={onaddtext}>
			<Type class="size-4" /> เพิ่มข้อความ
		</Button>
		<Button
			type="button"
			size="sm"
			variant="outline"
			disabled={disabled || hasQr}
			onclick={onaddqr}
			title={hasQr ? 'หนึ่งแบบมี QR Code ได้หนึ่งรายการ' : undefined}
		>
			<QrCode class="size-4" /> เพิ่ม QR Code
		</Button>
	</div>

	<div class="rounded-lg border bg-muted/15 p-3">
		<label for="certificate-image-asset" class="text-xs font-medium">เพิ่มรูปภาพ</label>
		<div class="mt-2 flex gap-2">
			<select
				id="certificate-image-asset"
				class="h-9 min-w-0 flex-1 rounded-md border bg-background px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
				bind:value={imageAssetId}
				disabled={disabled || imageAssets.length === 0}
			>
				<option value="">เลือกรูปที่แนบไว้</option>
				{#each imageAssets as asset (asset.id)}
					<option value={asset.id}>{asset.displayName}</option>
				{/each}
			</select>
			<Button
				type="button"
				size="icon-sm"
				variant="outline"
				disabled={disabled || !imageAssetId}
				onclick={addSelectedImage}
				aria-label="เพิ่มรูปภาพที่เลือก"
			>
				<ImagePlus class="size-4" />
			</Button>
		</div>
		{#if imageAssets.length === 0}
			<p class="mt-2 text-[0.68rem] text-amber-700">แนบรูปในหน้าจัดการไฟล์ของแบบนี้ก่อน</p>
		{/if}
	</div>

	<div class="border-t"></div>

	{#if !selectedElement}
		<div class="rounded-xl border border-dashed bg-muted/10 px-4 py-8 text-center">
			<Braces class="mx-auto size-7 text-muted-foreground/50" />
			<p class="mt-2 text-xs font-medium">เลือกองค์ประกอบบนกระดาษ</p>
			<p class="mt-1 text-[0.7rem] text-muted-foreground">เพื่อแก้ข้อความ ขนาด สี และตำแหน่ง</p>
		</div>
	{:else}
		<div class="space-y-4">
			<div class="flex items-center justify-between gap-2">
				<div class="flex items-center gap-2 text-xs font-semibold">
					{#if selectedElement.type === 'text'}
						<Type class="size-4 text-primary" /> ข้อความ
					{:else if selectedElement.type === 'image'}
						<ImagePlus class="size-4 text-primary" /> รูปภาพ
					{:else}
						<QrCode class="size-4 text-primary" /> QR Code
					{/if}
				</div>
				<div class="flex">
					<Button
						type="button"
						size="icon-sm"
						variant="ghost"
						{disabled}
						onclick={onduplicate}
						aria-label="ทำสำเนา"
					>
						<Copy class="size-3.5" />
					</Button>
					<Button
						type="button"
						size="icon-sm"
						variant="ghost"
						{disabled}
						onclick={ondelete}
						aria-label="ลบองค์ประกอบ"
					>
						<Trash2 class="size-3.5" />
					</Button>
				</div>
			</div>

			{#if selectedElement.type === 'text'}
				<div class="space-y-2">
					<label for="certificate-text-content" class="text-xs font-medium">ข้อความ</label>
					<Textarea
						id="certificate-text-content"
						rows={4}
						value={selectedElement.content}
						{disabled}
						oninput={(event) => patchText({ content: event.currentTarget.value })}
					/>
					<CertificateVariablePicker
						{variables}
						{disabled}
						oninsert={(token) => patchText({ content: `${selectedElement.content}${token}` })}
					/>
				</div>

				<div class="grid grid-cols-[minmax(0,1fr)_6.5rem] gap-2">
					<label class="space-y-1.5 text-xs">
						<span class="font-medium">ตระกูลฟอนต์</span>
						<select
							class="h-9 w-full rounded-md border bg-background px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
							value={currentFontVariant?.familyKey ?? ''}
							{disabled}
							onchange={(event) => changeFontFamily(event.currentTarget.value)}
						>
							{#each fontFamilies as family (family.familyKey)}
								<option value={family.familyKey}>{family.familyLabel}</option>
							{/each}
						</select>
					</label>
					<label class="space-y-1.5 text-xs">
						<span class="font-medium">น้ำหนักฟอนต์</span>
						<select
							class="h-9 w-full rounded-md border bg-background px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
							value={String(currentFontVariant?.weight ?? '')}
							{disabled}
							onchange={(event) => changeFontWeight(event.currentTarget.value)}
						>
							{#each currentFontWeights as weight (weight)}
								<option value={String(weight)}>{weight}</option>
							{/each}
						</select>
					</label>
				</div>
				<div class="grid grid-cols-2 gap-2">
					<Button
						type="button"
						size="sm"
						variant={currentFontVariant?.weight === 700 ? 'secondary' : 'outline'}
						disabled={disabled || !boldVariant}
						onclick={() => applyFontVariant(boldVariant)}
						title={boldVariant
							? 'สลับน้ำหนัก 700/ปกติ'
							: 'ยังไม่มี variant น้ำหนัก 700 สำหรับรูปแบบนี้'}
					>
						ตัวหนา
					</Button>
					<Button
						type="button"
						size="sm"
						variant={currentFontVariant?.style === 'italic' ? 'secondary' : 'outline'}
						disabled={disabled || !italicVariant}
						onclick={() => applyFontVariant(italicVariant)}
						title={italicVariant
							? 'สลับตัวปกติ/ตัวเอียง'
							: 'ยังไม่มี variant ตัวเอียงหรือตัวปกติที่น้ำหนักนี้'}
					>
						ตัวเอียง
					</Button>
				</div>

				<div class="grid grid-cols-2 gap-3">
					<label class="space-y-1.5 text-xs">
						<span class="font-medium">ขนาดตัวอักษร</span>
						<Input
							type="number"
							min="6"
							max="200"
							step="0.5"
							value={selectedElement.fontSize}
							{disabled}
							onchange={(event) =>
								patchText({ fontSize: Math.max(6, finiteInput(event, selectedElement.fontSize)) })}
						/>
					</label>
					<label class="space-y-1.5 text-xs">
						<span class="font-medium">สีข้อความ</span>
						<Input
							type="color"
							value={selectedElement.color}
							{disabled}
							oninput={(event) => patchText({ color: event.currentTarget.value })}
							class="p-1"
						/>
					</label>
					<label class="space-y-1.5 text-xs">
						<span class="font-medium">ระยะบรรทัด</span>
						<Input
							type="number"
							min="0.8"
							max="3"
							step="0.05"
							value={selectedElement.lineHeight}
							{disabled}
							onchange={(event) =>
								patchText({
									lineHeight: Math.max(0.8, finiteInput(event, selectedElement.lineHeight))
								})}
						/>
					</label>
					<label class="space-y-1.5 text-xs">
						<span class="font-medium">ขนาดต่ำสุด</span>
						<Input
							type="number"
							min="6"
							max={selectedElement.fontSize}
							step="0.5"
							value={selectedElement.minFontSize}
							disabled={disabled || !selectedElement.autoShrink}
							onchange={(event) =>
								patchText({
									minFontSize: Math.min(
										selectedElement.fontSize,
										Math.max(6, finiteInput(event, selectedElement.minFontSize))
									)
								})}
						/>
					</label>
				</div>

				<div class="space-y-2">
					<p class="text-xs font-medium">จัดแนว</p>
					<div class="grid grid-cols-3 rounded-md border p-0.5">
						{#each [{ value: 'left', label: 'ชิดซ้าย', icon: AlignLeft }, { value: 'center', label: 'กึ่งกลาง', icon: AlignCenter }, { value: 'right', label: 'ชิดขวา', icon: AlignRight }] as action (action.value)}
							<Button
								type="button"
								size="sm"
								variant={selectedElement.alignment === action.value ? 'secondary' : 'ghost'}
								{disabled}
								onclick={() =>
									patchText({ alignment: action.value as TextCertificateElement['alignment'] })}
								aria-label={action.label}
							>
								<action.icon class="size-4" />
							</Button>
						{/each}
					</div>
				</div>

				<div class="space-y-3 rounded-lg border bg-muted/15 p-3">
					<label class="flex items-center justify-between gap-3 text-xs font-medium">
						<span>ย่ออัตโนมัติเมื่อข้อความยาว</span>
						<input
							type="checkbox"
							checked={selectedElement.autoShrink}
							{disabled}
							onchange={(event) => patchText({ autoShrink: event.currentTarget.checked })}
							class="size-4 rounded border"
						/>
					</label>
					<label class="flex items-center justify-between gap-3 text-xs font-medium">
						<span>เงา</span>
						<input
							type="checkbox"
							checked={selectedElement.shadow !== null && selectedElement.shadow !== undefined}
							{disabled}
							onchange={(event) => toggleShadow(event.currentTarget.checked)}
							class="size-4 rounded border"
						/>
					</label>
					{#if selectedElement.shadow}
						<div class="grid grid-cols-2 gap-2">
							<label class="space-y-1 text-[0.68rem]">
								<span>เยื้อง X</span>
								<Input
									type="number"
									step="0.5"
									value={selectedElement.shadow.offsetX}
									{disabled}
									onchange={(event) =>
										patchText({
											shadow: {
												...selectedElement.shadow!,
												offsetX: finiteInput(event, selectedElement.shadow!.offsetX)
											}
										})}
								/>
							</label>
							<label class="space-y-1 text-[0.68rem]">
								<span>เยื้อง Y</span>
								<Input
									type="number"
									step="0.5"
									value={selectedElement.shadow.offsetY}
									{disabled}
									onchange={(event) =>
										patchText({
											shadow: {
												...selectedElement.shadow!,
												offsetY: finiteInput(event, selectedElement.shadow!.offsetY)
											}
										})}
								/>
							</label>
							<label class="space-y-1 text-[0.68rem]">
								<span>ความฟุ้ง</span>
								<Input
									type="number"
									min="0"
									step="0.5"
									value={selectedElement.shadow.blur}
									{disabled}
									onchange={(event) =>
										patchText({
											shadow: {
												...selectedElement.shadow!,
												blur: Math.max(0, finiteInput(event, selectedElement.shadow!.blur))
											}
										})}
								/>
							</label>
							<label class="space-y-1 text-[0.68rem]">
								<span>สีเงา</span>
								<Input
									type="color"
									value={selectedElement.shadow.color.slice(0, 7)}
									{disabled}
									oninput={(event) =>
										patchText({
											shadow: { ...selectedElement.shadow!, color: event.currentTarget.value }
										})}
									class="p-1"
								/>
							</label>
						</div>
					{/if}
				</div>
			{:else if selectedElement.type === 'image'}
				<div class="space-y-2">
					<label for="certificate-selected-image" class="text-xs font-medium">ไฟล์รูปภาพ</label>
					<select
						id="certificate-selected-image"
						class="h-9 w-full rounded-md border bg-background px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
						value={selectedElement.assetId}
						{disabled}
						onchange={(event) => changeSelectedImage(event.currentTarget.value)}
					>
						{#each imageAssets as asset (asset.id)}
							<option value={asset.id}>{asset.displayName}</option>
						{/each}
					</select>
				</div>
				<div class="space-y-3 rounded-lg border bg-muted/15 p-3">
					<label class="flex items-center justify-between gap-3 text-xs font-medium">
						<span>ล็อกสัดส่วน</span>
						<input
							type="checkbox"
							checked={selectedElement.lockAspectRatio}
							{disabled}
							onchange={(event) => changeImageAspectLock(event.currentTarget.checked)}
							class="size-4 rounded border"
						/>
					</label>
					<Button
						type="button"
						size="sm"
						variant="outline"
						class="w-full"
						{disabled}
						onclick={resetSelectedImageRatio}
					>
						รีเซ็ตสัดส่วนต้นฉบับ
					</Button>
				</div>
			{:else}
				<div
					class="rounded-lg border border-blue-200 bg-blue-50 p-3 text-xs leading-relaxed text-blue-950"
				>
					QR Code ใช้รหัสตรวจสอบจากระบบโดยอัตโนมัติ และกำหนดระดับแก้ข้อผิดพลาด M
				</div>
			{/if}

			<div class="space-y-3 rounded-lg border p-3">
				<p class="text-xs font-semibold">ตำแหน่งและขนาด (points)</p>
				<div class="grid grid-cols-2 gap-2">
					{#each [{ key: 'x', label: 'X' }, { key: 'y', label: 'Y' }, { key: 'width', label: 'กว้าง' }, { key: 'height', label: 'สูง' }] as field (field.key)}
						<label class="space-y-1 text-[0.68rem]">
							<span>{field.label}</span>
							<Input
								type="number"
								min="0"
								step="0.5"
								value={selectedElement.frame[field.key as keyof typeof selectedElement.frame]}
								{disabled}
								onchange={(event) =>
									patchFrame(
										field.key as 'x' | 'y' | 'width' | 'height',
										finiteInput(
											event,
											selectedElement.frame[field.key as keyof typeof selectedElement.frame]
										)
									)}
							/>
						</label>
					{/each}
					<label class="col-span-2 space-y-1 text-[0.68rem]">
						<span>หมุน (องศา)</span>
						<Input
							type="number"
							step="1"
							value={selectedElement.rotation}
							{disabled}
							onchange={(event) => patchRotation(finiteInput(event, selectedElement.rotation))}
						/>
					</label>
				</div>
			</div>
		</div>
	{/if}
</section>
