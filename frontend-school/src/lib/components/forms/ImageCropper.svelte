<script lang="ts">
	import CropperOriginal from 'svelte-easy-crop';
	const Cropper = CropperOriginal as any;
	import { Button } from '$lib/components/ui/button';
	import {
		Dialog,
		DialogContent,
		DialogHeader,
		DialogTitle,
		DialogFooter
	} from '$lib/components/ui/dialog';
	import { LoaderCircle } from 'lucide-svelte';

	interface Props {
		open: boolean;
		imageSrc: string | null;
		aspect?: number;
		onCropComplete: (croppedImage: Blob) => void;
	}

	let {
		open = $bindable(false),
		imageSrc = null,
		aspect = 1,
		onCropComplete
	}: Props = $props();

	let crop = $state({ x: 0, y: 0 });
	let zoom = $state(1);
	let croppedAreaPixels = $state<{ x: number; y: number; width: number; height: number } | null>(
		null
	);
	let processing = $state(false);

	function handleCropComplete(detail: {
		percent: { x: number; y: number; width: number; height: number };
		pixels: { x: number; y: number; width: number; height: number };
	}) {
		croppedAreaPixels = detail.pixels;
	}

	async function handleSave() {
		if (!imageSrc || !croppedAreaPixels) return;

		try {
			processing = true;
			const croppedImage = await getCroppedImg(imageSrc, croppedAreaPixels);
			if (croppedImage) {
				onCropComplete(croppedImage);
				open = false; // Close dialog via binding
			}
		} catch (e) {
			console.error(e);
		} finally {
			processing = false;
		}
	}

	// Utility function to crop image using canvas
	async function getCroppedImg(
		imageSrc: string,
		pixelCrop: { x: number; y: number; width: number; height: number }
	): Promise<Blob | null> {
		const image = await createImage(imageSrc);
		const canvas = document.createElement('canvas');
		const ctx = canvas.getContext('2d');

		if (!ctx) {
			return null;
		}

		canvas.width = pixelCrop.width;
		canvas.height = pixelCrop.height;

		ctx.drawImage(
			image,
			pixelCrop.x,
			pixelCrop.y,
			pixelCrop.width,
			pixelCrop.height,
			0,
			0,
			pixelCrop.width,
			pixelCrop.height
		);

		return new Promise((resolve) => {
			canvas.toBlob((blob) => {
				resolve(blob);
			}, 'image/jpeg');
		});
	}

	function createImage(url: string): Promise<HTMLImageElement> {
		return new Promise((resolve, reject) => {
			const image = new Image();
			image.addEventListener('load', () => resolve(image));
			image.addEventListener('error', (error) => reject(error));
			image.src = url;
		});
	}
</script>

<Dialog bind:open>
	<DialogContent class="sm:max-w-[500px]">
		<DialogHeader>
			<DialogTitle>ปรับแต่งรูปโปรไฟล์</DialogTitle>
		</DialogHeader>

		<div class="relative w-full h-[400px] bg-black/5 rounded-md overflow-hidden">
			{#if imageSrc}
				<Cropper
					image={imageSrc}
					bind:crop
					bind:zoom
					{aspect}
					on:cropcomplete={(e: any) => handleCropComplete(e.detail)}
				/>
			{/if}
		</div>

		<div class="py-2">
			<div class="flex items-center justify-between text-sm text-muted-foreground mb-2">
				<span>ซูม</span>
				<span>{zoom.toFixed(1)}x</span>
			</div>
			<input
				type="range"
				min="1"
				max="3"
				step="0.1"
				bind:value={zoom}
				class="w-full h-2 bg-secondary rounded-lg appearance-none cursor-pointer accent-primary"
			/>
		</div>

		<DialogFooter>
			<Button variant="outline" onclick={() => (open = false)} disabled={processing}>ยกเลิก</Button>
			<Button onclick={handleSave} disabled={processing}>
				{#if processing}
					<LoaderCircle class="mr-2 h-4 w-4 animate-spin" />
					กำลังบันทึก...
				{:else}
					บันทึกรูปภาพ
				{/if}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>
