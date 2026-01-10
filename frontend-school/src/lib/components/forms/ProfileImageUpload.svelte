<script lang="ts">
	import { type Snippet } from 'svelte';
	import ImageUpload from './ImageUpload.svelte';
	import { uploadProfileImage } from '$lib/api/files';
	import { toast } from 'svelte-sonner';

	interface Props {
		currentImage?: string | null;
		disabled?: boolean;
		maxSizeMB?: number;
		onsuccess?: (data: { url: string; fileId: string }) => void;
		onerror?: (error: string) => void;
		helper?: Snippet;
	}

	let {
		currentImage = null,
		disabled = false,
		maxSizeMB = 5,
		onsuccess,
		onerror,
		helper
	}: Props = $props();

	// State
	let uploading = $state(false);
	let imageUrl = $state(currentImage);

	// Update when prop changes
	$effect(() => {
		if (currentImage !== imageUrl) {
			imageUrl = currentImage;
		}
	});

	// Handle file upload
	async function handleUpload(file: File) {
		uploading = true;

		try {
			const response = await uploadProfileImage(file);

			if (response.success) {
				imageUrl = response.file.url;
				toast.success('อัปโหลดรูปภาพสำเร็จ');

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

	// Handle remove
	function handleRemove() {
		imageUrl = null;
		onsuccess?.({ url: '', fileId: '' });
	}
</script>

<ImageUpload
	value={imageUrl}
	{maxSizeMB}
	{disabled}
	onupload={handleUpload}
	onremove={handleRemove}
	{helper}
/>
