<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import ImageUpload from './ImageUpload.svelte';
	import { uploadProfileImage } from '$lib/api/files';
	import { toast } from 'svelte-sonner';

	// Props
	export let currentImage: string | null = null;
	export let disabled: boolean = false;
	export let maxSizeMB: number = 5;

	// State
	let uploading = false;
	let imageUrl = currentImage;

	const dispatch = createEventDispatcher<{
		success: { url: string; fileId: string };
		error: string;
	}>();

	// Handle file upload
	async function handleUpload(event: CustomEvent<File>) {
		const file = event.detail;
		uploading = true;

		try {
			const response = await uploadProfileImage(file);

			if (response.success) {
				imageUrl = response.file.url;
				toast.success('อัปโหลดรูปภาพสำเร็จ');

				dispatch('success', {
					url: response.file.url,
					fileId: response.file.id
				});
			}
		} catch (error) {
			const errorMessage = error instanceof Error ? error.message : 'เกิดข้อผิดพลาด';
			toast.error(`ไม่สามารถอัปโหลดรูปภาพได้: ${errorMessage}`);
			dispatch('error', errorMessage);
		} finally {
			uploading = false;
		}
	}

	// Handle remove
	function handleRemove() {
		imageUrl = null;
		dispatch('success', { url: '', fileId: '' });
	}

	// Update when prop changes
	$: if (currentImage !== imageUrl) {
		imageUrl = currentImage;
	}
</script>

<ImageUpload
	value={imageUrl}
	{maxSizeMB}
	{disabled}
	on:upload={handleUpload}
	on:remove={handleRemove}
	{...$$restProps}
>
	<slot slot="helper" name="helper" />
</ImageUpload>
