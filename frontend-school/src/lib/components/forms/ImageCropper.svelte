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
	let zoom = $state(1); // 1 = fits container (base scale)
	let processing = $state(false);
	
	// Image State
    // imgElement: reference to the <img> tag
	let imgElement = $state<HTMLImageElement>();
    // containerElement: reference to the wrapper div
	let containerElement = $state<HTMLDivElement>();
    
    // Internal calculations
    let baseScale = $state(1); // Scale to fit the image inside container completely
    let naturalWidth = 0;
    let naturalHeight = 0;
    
    // Pan State (x, y translation relative to center)
	let position = $state({ x: 0, y: 0 });
	let dragging = $state(false);
	let dragStart = { x: 0, y: 0 };

    // Dynamic Mask Size
    let maskSize = $state(250); 

    // Reset when image source changes
    $effect(() => {
        if (imageSrc) {
            position = { x: 0, y: 0 };
            zoom = 1;
            // baseScale will be calculated in onImageLoad
        }
    });

    function onImageLoad() {
        if (!imgElement || !containerElement) return;

        naturalWidth = imgElement.naturalWidth;
        naturalHeight = imgElement.naturalHeight;
        const cw = containerElement.clientWidth;
        const ch = containerElement.clientHeight;

        // Dynamic Mask Size: 90% of the smallest dimension of the container
        maskSize = Math.min(cw, ch) * 0.9;

        // Calculate base scale to COVER the mask
        // We want the shortest side of the image to match the mask diameter.
        const scaleX = maskSize / naturalWidth;
        const scaleY = maskSize / naturalHeight;
        
        // Use Math.max for COVER logic (so no black borders initially)
        baseScale = Math.max(scaleX, scaleY); 
        
        // Center image initially (position 0,0 is center because of logic below)
        position = { x: 0, y: 0 };
        zoom = 1; 
    }

	function handleMouseDown(e: MouseEvent) {
		e.preventDefault();
		dragging = true;
		dragStart = { x: e.clientX - position.x, y: e.clientY - position.y };
		window.addEventListener('mousemove', handleMouseMove);
		window.addEventListener('mouseup', handleMouseUp);
	}

	function getConstraints() {
        if (!naturalWidth || !naturalHeight) return { x: 0, y: 0 };
        
        // Current rendered dimensions
        const currentScale = baseScale * zoom;
        const renderW = naturalWidth * currentScale;
        const renderH = naturalHeight * currentScale;
        
        // Panning is relative to center.
        // We can move the image until its edge hits the mask edge?
        // Actually, for "cover", we usually want the image to cover the mask completely?
        // Or cover the container?
        // Let's assume we want the image to cover the MASK area at least.
        
        // Max displacement is half the difference between Image Size and Mask Size
        const maxX = Math.max(0, (renderW - maskSize) / 2);
        const maxY = Math.max(0, (renderH - maskSize) / 2);
        
        return { x: maxX, y: maxY };
    }

	function handleMouseMove(e: MouseEvent) {
        if (!dragging) return;
        
        let newX = e.clientX - dragStart.x;
        let newY = e.clientY - dragStart.y;
        
        // Apply Constraints (Prevent edges from entering mask)
        const limits = getConstraints();
        newX = Math.max(-limits.x, Math.min(newX, limits.x));
        newY = Math.max(-limits.y, Math.min(newY, limits.y));
        
        position.x = newX;
        position.y = newY;
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
         if (e.cancelable) e.preventDefault(); // Prevent scrolling
         const touch = e.touches[0];
         
         let newX = touch.clientX - dragStart.x;
         let newY = touch.clientY - dragStart.y;
         
         const limits = getConstraints();
         newX = Math.max(-limits.x, Math.min(newX, limits.x));
         newY = Math.max(-limits.y, Math.min(newY, limits.y));
         
         position.x = newX;
         position.y = newY;
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
			const croppedImage = await getCroppedImg();
			if (croppedImage) {
				onCropComplete(croppedImage);
				open = false;
			} else {
                console.error("Failed to crop image: blob is null");
            }
		} catch (e) {
			console.error("Critical error in handleSave:", e);
		} finally {
			processing = false;
		}
	}
    
	// Utility function to crop image using canvas
	function getCroppedImg(): Promise<Blob | null> {
        return new Promise((resolve) => {
             if (!imgElement || !containerElement) {
                 resolve(null);
                 return;
            }

            const canvas = document.createElement('canvas');
            const ctx = canvas.getContext('2d');
            if (!ctx) {
                resolve(null);
                return;
            }

            // Output size for avatar
            const size = 512;
            canvas.width = size;
            canvas.height = size;
            
            // --- CROP LOGIC ---
            // 1. Current Displayed Properties
            // The image is displayed with scale = baseScale * zoom
            const totalScale = baseScale * zoom;
            
            // 2. Center Points
            // We interpret position (x, y) as the displacement of the Image Center from the Container Center.
            // Mask Center is always at Container Center.
            // So: Image Center = Mask Center + (x, y)
            // Or: Vector(Image -> Mask) = (-x, -y)
            
            // 3. Mapping Mask Center to Natural Image Coordinates
            // Natural Center = (naturalWidth / 2, naturalHeight / 2)
            // Displacement in Natural Pixels = (-x / totalScale, -y / totalScale)
            
            const centerNatX = naturalWidth / 2;
            const centerNatY = naturalHeight / 2;
            
            const maskCenterInNatX = centerNatX - (position.x / totalScale);
            const maskCenterInNatY = centerNatY - (position.y / totalScale);
            
            // 4. Determining the Crop Region in Natural Pixels
            // We want to crop an area corresponding to maskSize pixels on screen.
            // Dimension in Natural Pixels = maskSize / totalScale
            
            const cropNatSize = maskSize / totalScale;
            
            const cropNatX = maskCenterInNatX - (cropNatSize / 2);
            const cropNatY = maskCenterInNatY - (cropNatSize / 2);
            
            // 5. Draw
            // Debug logs
            // console.log({ totalScale, position, cropNatX, cropNatY, cropNatSize, naturalWidth, naturalHeight });

            ctx.fillStyle = '#FFFFFF'; // Fill background white/transparent just in case
            ctx.fillRect(0, 0, size, size);
            
            ctx.drawImage(
                imgElement,
                cropNatX, cropNatY, cropNatSize, cropNatSize, // Source
                0, 0, size, size // Destination
            );
            
            canvas.toBlob((blob) => {
                resolve(blob);
            }, 'image/jpeg', 0.95);
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
				<img
					bind:this={imgElement}
					src={imageSrc}
					alt="Crop target"
					class="absolute max-w-none transition-transform duration-75 ease-linear pointer-events-none origin-center"
					style:transform="translate({position.x}px, {position.y}px) scale({baseScale * zoom})"
					onload={onImageLoad}
					ondragstart={(e) => e.preventDefault()}
				/>

				<!-- Overlay (Circular Mask) -->
				<!-- Mask size is dynamic -->
				<div class="absolute inset-0 pointer-events-none z-10 flex items-center justify-center">
					<div
						class="rounded-full border-2 border-white/50 shadow-[0_0_0_9999px_rgba(0,0,0,0.8)]"
						style:width="{maskSize}px"
						style:height="{maskSize}px"
					></div>
					<!-- Slight dashed border for better UI visibility -->
					<div
						class="absolute rounded-full border border-dashed border-white/40"
						style:width="{maskSize}px"
						style:height="{maskSize}px"
					></div>
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
				step="0.01"
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
