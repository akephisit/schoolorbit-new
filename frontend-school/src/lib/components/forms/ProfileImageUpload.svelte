<script lang="ts">
	import { browser } from '$app/env';
	import { onDestroy, untrack, type Snippet } from 'svelte';
	import { uploadFile } from '#lib/api/files.js';
	import { PendingUpload } from '#lib/files/pending-upload.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import PendingFileGuard from '#lib/components/files/PendingFileGuard.svelte';
	import { toast } from 'svelte-sonner';
	import { Pencil, Camera, Trash2, UserCircle, LoaderCircle } from '@lucide/svelte';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import PrivateFileImage from '#lib/components/files/PrivateFileImage.svelte';
	import { cn } from '#lib/utils.js';

	interface Props {
		size?: 'sm' | 'md';
		currentFileId?: string | null;
		resourceId?: string;
		disabled?: boolean;
		maxSizeMB?: number;
		onsuccess?: (data: { fileId: string | null }) => Promise<void> | void;
		onerror?: (error: string) => void;
		helper?: Snippet;
		className?: string;
	}

	let {
		size = 'md',
		currentFileId = null,
		resourceId,
		disabled = false,
		maxSizeMB = 10,
		onsuccess,
		onerror,
		helper,
		className = ''
	}: Props = $props();

	// State
	let uploading = $state(false);
	let localFileId = $state<string | null | undefined>(undefined);
	let imageFileId = $derived(localFileId === undefined ? currentFileId : localFileId);
	let showCropper = $state(false);
	let ImageCropper = $state<typeof import('./ImageCropper.svelte').default>();
	let tempImageSrc = $state<string | null>(null);
	let fileInput = $state<HTMLInputElement>();
	let selectionEpoch = 0;

	// 1. Handle File Selection
	async function handleFileSelect(event: Event) {
		if (uploading || disabled || disposed) return;
		const owner = resourceId,
			revision = ++selectionEpoch;
		const current = () =>
			!disposed && owner === resourceId && revision === selectionEpoch && !disabled;
		const target = event.target as HTMLInputElement;
		let file = target.files?.[0];

		if (file) {
			const isHeic =
				file.name.toLowerCase().endsWith('.heic') ||
				file.type === 'image/heic' ||
				file.type === 'image/heif';

			// Validate type
			if (!isHeic && !file.type.startsWith('image/')) {
				toast.error('กรุณาเลือกไฟล์รูปภาพเท่านั้น');
				return;
			}

			if (file.size > maxSizeMB * 1024 * 1024) {
				toast.error(`ไฟล์ต้นฉบับต้องไม่เกิน ${maxSizeMB}MB`);
				return;
			}

			// Loading toast removed as requested

			try {
				if (isHeic) {
					if (!browser) throw new Error('HEIC conversion requires a browser');
					const { default: heic2any } = await import('heic2any');
					const result = await heic2any({
						blob: file,
						toType: 'image/jpeg',
						quality: 0.8
					});

					const blob = Array.isArray(result) ? result[0] : result;
					file = new File([blob], file.name.replace(/\.heic$/i, '.jpg'), {
						type: 'image/jpeg',
						lastModified: Date.now()
					});
				}

				const [{ default: Compressor }, { default: Cropper }] = await Promise.all([
					import('compressorjs'),
					import('./ImageCropper.svelte')
				]);
				if (!current()) return;
				ImageCropper = Cropper;
				// Optimize orientation only after selection.
				new Compressor(file, {
					quality: 0.8,
					maxWidth: 1920,
					maxHeight: 1920,
					mimeType: 'image/jpeg',
					success(result: Blob | File) {
						if (!current()) return;
						const reader = new FileReader();
						reader.onload = (e) => {
							if (!current()) return;
							tempImageSrc = e.target?.result as string;
							showCropper = true;
							target.value = '';
						};
						reader.readAsDataURL(result);
					},
					error(err: Error) {
						if (!current()) return;
						console.error('Compressor error:', err);
						// Fallback to original
						if (file) {
							const reader = new FileReader();
							reader.onload = (e) => {
								if (!current()) return;
								tempImageSrc = e.target?.result as string;
								showCropper = true;
								target.value = '';
							};
							reader.readAsDataURL(file);
						}
					}
				});
			} catch (e) {
				console.error('File processing error:', e);
				// toast.error('ไม่สามารถประมวลผลรูปภาพได้'); // Keep error toast or remove per preference? keeping error is usually safe.
			}
			// finally block removed as toastId is gone
		}
	}

	let pendingImage = $state<PendingUpload>();
	let pendingFileId = $state<string | null | undefined>(undefined),
		commitError = $state('');
	let disposed = false,
		generation = 0,
		activeOwner = untrack(() => resourceId);
	$effect(() => {
		const owner = resourceId;
		untrack(() => {
			if (activeOwner !== owner) {
				generation++;
				pendingImage?.dispose();
				pendingImage = undefined;
				activeOwner = owner;
				selectionEpoch++;
				uploading = false;
				pendingFileId = undefined;
				commitError = '';
				localFileId = undefined;
				showCropper = false;
				tempImageSrc = null;
			}
		});
	});
	onDestroy(() => {
		disposed = true;
		selectionEpoch++;
		generation++;
		pendingImage?.dispose();
	});
	async function persistPhoto() {
		if (uploading || disabled) return;
		const owner = resourceId,
			revision = generation;
		const current = () => !disposed && owner === resourceId && revision === generation;
		uploading = true;
		commitError = '';
		try {
			let id = pendingFileId;
			if (id === undefined && pendingImage) id = await pendingImage.commit();
			if (!current() || id === undefined) return;
			pendingFileId = id;
			await onsuccess?.({ fileId: id });
			if (!current()) return;
			localFileId = id;
			pendingFileId = undefined;
			pendingImage?.dispose();
			pendingImage = undefined;
		} catch (cause) {
			if (!current()) return;
			commitError = cause instanceof Error ? cause.message : 'บันทึกรูปภาพไม่สำเร็จ';
			onerror?.(commitError);
		} finally {
			if (current()) uploading = false;
		}
	}
	async function handleCropComplete(blob: Blob) {
		if (uploading || disabled) return;
		pendingImage?.dispose();
		generation++;
		pendingImage = new PendingUpload(
			new File([blob], 'profile_avatar.jpg', { type: 'image/jpeg' }),
			{
				purpose: 'profile_image',
				resourceId,
				owner: resourceId ?? 'self'
			},
			uploadFile
		);
		pendingFileId = undefined;
		await persistPhoto();
	}
	async function handleRemove() {
		if (uploading || disabled) return;
		generation++;
		pendingImage?.dispose();
		pendingImage = undefined;
		pendingFileId = null;
		await persistPhoto();
	}

	function triggerFileInput() {
		fileInput?.click();
	}

	function captureFileInput(node: HTMLInputElement) {
		fileInput = node;
		return () => {
			if (fileInput === node) fileInput = undefined;
		};
	}
</script>

<PendingFileGuard dirty={!!pendingImage || pendingFileId !== undefined} pending={uploading} />
<div class={cn('flex flex-col gap-4', className)}>
	<div class="relative group w-fit mx-auto">
		<!-- Profile Avatar Circle -->
		<div
			class={cn(
				'relative shrink-0 rounded-full overflow-hidden ring-4 ring-background shadow-lg bg-muted flex items-center justify-center',
				size === 'sm' ? 'size-20' : 'size-32'
			)}
		>
			{#if uploading}
				<div
					class="absolute inset-0 bg-black/50 z-10 flex flex-col items-center justify-center text-white"
				>
					<LoaderCircle class="w-8 h-8 animate-spin mb-2" />
					<span class="text-xs font-medium">กำลังบันทึกรูปภาพ</span>
				</div>
			{/if}

			{#if imageFileId}
				<PrivateFileImage
					fileId={imageFileId}
					{resourceId}
					alt="Profile"
					class="w-full h-full object-cover"
				/>
			{:else}
				<UserCircle class="w-20 h-20 text-muted-foreground/50" />
			{/if}
		</div>

		<!-- Edit Button (Dropdown Trigger) -->
		{#if !disabled && !uploading}
			<DropdownMenu.Root>
				<DropdownMenu.Trigger class="absolute bottom-0 right-0 outline-none">
					<div
						class="w-10 h-10 rounded-full bg-background border border-border shadow-sm flex items-center justify-center hover:bg-accent transition-colors cursor-pointer"
						title="แก้ไขรูปโปรไฟล์"
					>
						<Pencil class="w-5 h-5 text-foreground" />
					</div>
				</DropdownMenu.Trigger>

				<DropdownMenu.Content align="start" side="right" class="w-48">
					<DropdownMenu.Item onclick={triggerFileInput} class="cursor-pointer">
						<Camera class="w-4 h-4 mr-2" />
						<span>อัปโหลดรูปภาพ...</span>
					</DropdownMenu.Item>

					{#if imageFileId}
						<DropdownMenu.Separator />
						<DropdownMenu.Item
							onclick={handleRemove}
							class="cursor-pointer text-destructive focus:text-destructive focus:bg-destructive/10"
						>
							<Trash2 class="w-4 h-4 mr-2" />
							<span>ลบรูปภาพ</span>
						</DropdownMenu.Item>
					{/if}
				</DropdownMenu.Content>
			</DropdownMenu.Root>
		{/if}
	</div>

	<!-- Hidden File Input -->
	<input
		{@attach captureFileInput}
		type="file"
		accept="image/png, image/jpeg, image/webp, .heic, image/heic, image/heif"
		class="hidden"
		onchange={handleFileSelect}
		{disabled}
	/>

	<!-- Helper Text -->
	{#if helper}
		{@render helper()}
	{:else}
		<p class="text-center text-sm text-muted-foreground mt-2">
			รองรับ: JPG, PNG, WebP, HEIC (สูงสุด {maxSizeMB}MB)
		</p>
	{/if}

	{#if commitError}<p role="alert" class="text-sm text-destructive">{commitError}</p>
		<Button type="button" variant="outline" disabled={uploading || disabled} onclick={persistPhoto}
			>ลองบันทึกรูปอีกครั้ง</Button
		>{/if}
	<!-- Image Cropper Modal -->
	{#if ImageCropper}<ImageCropper
			bind:open={showCropper}
			imageSrc={tempImageSrc}
			onCropComplete={handleCropComplete}
		/>{/if}
</div>
