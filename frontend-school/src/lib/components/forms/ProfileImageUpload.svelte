<script lang="ts">
	import { type Snippet } from 'svelte';
	import { uploadProfileImage } from '$lib/api/files';
	import { toast } from 'svelte-sonner';
	import { Pencil, Camera, Trash2, UserCircle, LoaderCircle } from 'lucide-svelte';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import ImageCropper from './ImageCropper.svelte';
	import { Button } from '$lib/components/ui/button';
	import { cn } from '$lib/utils';
	import heic2any from 'heic2any';
	import Compressor from 'compressorjs';

	interface Props {
		currentImage?: string | null;
		disabled?: boolean;
		maxSizeMB?: number;
		onsuccess?: (data: { url: string; fileId: string }) => void;
		onerror?: (error: string) => void;
		helper?: Snippet;
		className?: string;
	}

	let {
		currentImage = null,
		disabled = false,
		maxSizeMB = 5,
		onsuccess,
		onerror,
		helper,
		className = ''
	}: Props = $props();

	// State
	let uploading = $state(false);
	let imageUrl = $state(currentImage);
	let showCropper = $state(false);
	let tempImageSrc = $state<string | null>(null);
	let fileInput = $state<HTMLInputElement>();

	// Update when prop changes
	$effect(() => {
		if (currentImage !== imageUrl) {
			imageUrl = currentImage;
		}
	});

	// 1. Handle File Selection
	async function handleFileSelect(event: Event) {
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

			// Validate initial size (Allow up to 50MB for processing)
			if (file.size > 50 * 1024 * 1024) {
				toast.error(`ไฟล์ต้นฉบับต้องไม่เกิน 50MB`);
				return;
			}

			// Loading toast removed as requested

			try {
				if (isHeic) {
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

				// Optimize & Fix Orientation with Compressor.js
				// Optimize & Fix Orientation with Compressor.js
				new Compressor(file, {
					quality: 0.8,
					maxWidth: 1920,
					maxHeight: 1920,
					mimeType: 'image/jpeg',
					success(result: Blob | File) {
						const reader = new FileReader();
						reader.onload = (e) => {
							tempImageSrc = e.target?.result as string;
							showCropper = true;
							target.value = '';
						};
						reader.readAsDataURL(result);
					},
					error(err: Error) {
						console.error('Compressor error:', err);
						// Fallback to original
						if (file) {
							const reader = new FileReader();
							reader.onload = (e) => {
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

	// 2. Handle Cropped Image -> Upload
	async function handleCropComplete(croppedBlob: Blob) {
		uploading = true;

		// Convert Blob to File
		const file = new File([croppedBlob], 'profile_avatar.jpg', { type: 'image/jpeg' });

		try {
			const response = await uploadProfileImage(file);

			if (response.success) {
				imageUrl = response.file.url;

				onsuccess?.({
					url: response.file.url,
					fileId: response.file.id
				});
			}
		} catch (error) {
			const errorMessage = error instanceof Error ? error.message : 'เกิดข้อผิดพลาด';
			toast.error(`ไม่สามารถอัปโหลดรูปภาพได้: ${errorMessage}`);
			onerror?.(errorMessage);
		} finally {
			uploading = false;
		}
	}

	// Handle Remove
	function handleRemove() {
		imageUrl = null;
		onsuccess?.({ url: '', fileId: '' });
	}

	function triggerFileInput() {
		fileInput?.click();
	}
</script>

<div class={cn('flex flex-col gap-4', className)}>
	<div class="relative group w-fit mx-auto">
		<!-- Profile Avatar Circle -->
		<div
			class="relative w-32 h-32 rounded-full overflow-hidden ring-4 ring-background shadow-lg bg-muted flex items-center justify-center"
		>
			{#if uploading}
				<div
					class="absolute inset-0 bg-black/50 z-10 flex flex-col items-center justify-center text-white"
				>
					<LoaderCircle class="w-8 h-8 animate-spin mb-2" />
					<span class="text-xs font-medium">กำลังอัปโหลด</span>
				</div>
			{/if}

			{#if imageUrl}
				<img src={imageUrl} alt="Profile" class="w-full h-full object-cover" />
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

					{#if imageUrl}
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
		bind:this={fileInput}
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
			รองรับ: JPG, PNG, WebP, HEIC (สูงสุด 50MB)
		</p>
	{/if}

	<!-- Image Cropper Modal -->
	<ImageCropper
		bind:open={showCropper}
		imageSrc={tempImageSrc}
		aspect={1}
		onCropComplete={handleCropComplete}
	/>
</div>
