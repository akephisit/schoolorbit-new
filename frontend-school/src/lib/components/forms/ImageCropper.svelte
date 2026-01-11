<script lang="ts">
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

	// State
	let zoom = $state(1);
	let processing = $state(false);
	
	// Image State
    let imgElement = $state<HTMLImageElement>();
	let containerElement = $state<HTMLDivElement>();
    
    // Pan State (Relative to image center vs container center)
	let position = $state({ x: 0, y: 0 });
	let dragging = $state(false);
	let dragStart = { x: 0, y: 0 };

	function handleMouseDown(e: MouseEvent) {
		e.preventDefault();
		dragging = true;
		dragStart = { x: e.clientX - position.x, y: e.clientY - position.y };
		window.addEventListener('mousemove', handleMouseMove);
		window.addEventListener('mouseup', handleMouseUp);
	}

	function handleMouseMove(e: MouseEvent) {
        if (!dragging) return;
        // Basic Panning
        position.x = e.clientX - dragStart.x;
        position.y = e.clientY - dragStart.y;
        
        // เราสามารถเพิ่ม Boundary check ตรงนี้ได้ถ้าต้องการ แต่แบบ Free pan ก็ง่ายดี
	}

	function handleMouseUp() {
		dragging = false;
		window.removeEventListener('mousemove', handleMouseMove);
		window.removeEventListener('mouseup', handleMouseUp);
	}

    // Touch support (Basic)
    function handleTouchStart(e: TouchEvent) {
        if (e.touches.length !== 1) return;
        dragging = true;
        const touch = e.touches[0];
        dragStart = { x: touch.clientX - position.x, y: touch.clientY - position.y };
        window.addEventListener('touchmove', handleTouchMove);
        window.addEventListener('touchend', handleTouchEnd);
    }

     function handleTouchMove(e: TouchEvent) {
         if (!dragging) return;
         const touch = e.touches[0];
         position.x = touch.clientX - dragStart.x;
         position.y = touch.clientY - dragStart.y;
     }

     function handleTouchEnd() {
         dragging = false;
         window.removeEventListener('touchmove', handleTouchMove);
         window.removeEventListener('touchend', handleTouchEnd);
     }


	async function handleSave() {
		if (!imageSrc || !imgElement || !containerElement) return;

		try {
			processing = true;
			const croppedImage = await getCroppedImg(imgElement, position, zoom);
			if (croppedImage) {
				onCropComplete(croppedImage);
				open = false;
			}
		} catch (e) {
			console.error("Critical error in handleSave:", e);
		} finally {
			processing = false;
		}
	}
    
    // Reset position when image changes
    $effect(() => {
        if (imageSrc) {
            position = { x: 0, y: 0 };
            zoom = 1;
        }
    });

	// Utility function to crop image using canvas
	function getCroppedImg(
		image: HTMLImageElement,
        pos: { x: number, y: number },
        zoom: number
	): Promise<Blob | null> {
        return new Promise((resolve) => {
            if (!containerElement) {
                 resolve(null);
                 return;
            }
            const canvas = document.createElement('canvas');
            const ctx = canvas.getContext('2d');
            if (!ctx) {
                resolve(null);
                return;
            }

            // Output size (Avatar size usually 256x256 or 512x512)
            const size = 512;
            canvas.width = size;
            canvas.height = size;
            
            // Draw Logic
            // The container is like a viewport. The image is transformed inside it.
            // We want to capture what is inside the center of the container (the circular mask area).
            
            // 1. Calculate the image render size
            // Scale natural size to fit container then multiply by zoom
            // Assume container is 400x400 (from CSS)
            // But checking natural aspect ratio matters.
            
            // To simplify: let's assume 'contain' fit logic base + zoom + pan.
            // Image is rendered centered at (ContainerW/2 + pos.x, ContainerH/2 + pos.y)
            
            // Let's rely on the image's displayed rect relative to the container center.
            // Container Size:
            const containerW = containerElement.clientWidth;
            const containerH = containerElement.clientHeight;
            
            // Mask Area (Center of container)
            // Let's say mask is 80% of container or fixed size? 
            // In CSS I'll set mask to be circle in center.
            // Let's assume mask is min(containerW, containerH) * 0.8
            const maskSize = 250; // Fixed size for simplicity or calculated
            
            // Save context
            ctx.fillStyle = '#FFFFFF';
            ctx.fillRect(0, 0, size, size);
            
            // Calculate source rectangle from the image
            // Image is drawn at:
            // CenterX = containerW/2 + pos.x
            // CenterY = containerH/2 + pos.y
            // Scale = (image displayed width / natural width) ???
            // Let's use the actual transformations applied in CSS to calculating inverse.
            
            // Image CSS: transfrom: translate(x, y) scale(zoom)
            // And it is centered in flex container.
            
            // Easier way: Draw image to canvas with same transforms.
            
            // Clear
            ctx.clearRect(0, 0, size, size);
            
            // We want the area under the mask (size x size) to be drawn to canvas (size x size).
            // So we need to map container pixels to canvas pixels.
            // Mask in container is at center.
            
            // Canvas Center
            const cx = size / 2;
            const cy = size / 2;
            
            ctx.translate(cx, cy);
            // Apply image transformations relative to mask center
            // In container: Image Center is at (ContainerCenter + Position)
            // Mask Center is at ContainerCenter
            // So Image Center relative to Mask Center is (Position)
            
            // Warning: zoom is applied to the image.
            
            // Check aspect ratio to fit image initially (object-contain logic)
            const imgAspect = image.naturalWidth / image.naturalHeight;
            let renderW, renderH;
            
            // Assume initial fit is 'contain' within maskSize? No, usually 'cover' or 'fit'.
            // Let's assume initial render: Image fits inside the viewing area?
            // In the HTML below, I will render image with max-w-full max-h-full but initially centered.
            // Let's say we scale image so that min(w, h) = maskSize (cover).
            
            const scaleBase = Math.max(maskSize / image.naturalWidth, maskSize / image.naturalHeight);
            
            // Apply user zoom
            const scale = scaleBase * zoom;
            
            ctx.translate(position.x * (size/maskSize), position.y * (size/maskSize)); // Scale output movement?
            // Actually, if we map the mask directly to canvas:
            // 1 pixel on screen (mask) = (size / maskSize) pixels on canvas.
            // Ratio = size / maskSize.
            
            const ratio = size / maskSize;
            
            // Apply transforms
            ctx.translate(position.x * ratio, position.y * ratio);
            ctx.scale(scale * ratio, scale * ratio);
            
            // Draw image centered
            ctx.drawImage(image, -image.naturalWidth / 2, -image.naturalHeight / 2);
            
            canvas.toBlob((blob) => {
                resolve(blob);
            }, 'image/jpeg', 0.9);
        });
	}
</script>

<Dialog bind:open>
	<DialogContent class="sm:max-w-[500px]">
		<DialogHeader>
			<DialogTitle>ปรับแต่งรูปโปรไฟล์</DialogTitle>
		</DialogHeader>

		<!-- Container -->
		<div
			class="relative w-full h-[400px] bg-black/90 rounded-md overflow-hidden touch-none select-none flex items-center justify-center cursor-move"
			bind:this={containerElement}
			onmousedown={handleMouseDown}
			ontouchstart={handleTouchStart}
			role="presentation"
		>
			{#if imageSrc}
				<!-- Image -->
				<!-- We center it initially. transform handles pan & zoom -->
				<img
					bind:this={imgElement}
					src={imageSrc}
					alt="Crop target"
					class="absolute max-w-none transition-transform duration-75 ease-linear pointer-events-none"
					style:transform="translate({position.x}px, {position.y}px) scale({zoom})"
					style:transform-origin="center"
					style:min-width="250px"
					style:min-height="250px"
					ondragstart={(e) => e.preventDefault()}
				/>

				<!-- Overlay (Circular Mask) -->
				<!-- Use a huge border/shadow technique or SVG mask -->
				<!-- SVG Mask is safest for round hole -->
				<div class="absolute inset-0 pointer-events-none z-10 flex items-center justify-center">
					<div
						class="w-[250px] h-[250px] rounded-full border-2 border-white/50 shadow-[0_0_0_9999px_rgba(0,0,0,0.8)]"
					></div>
					<!-- Grid Lines (Optional) inside the circle -->
				</div>
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
