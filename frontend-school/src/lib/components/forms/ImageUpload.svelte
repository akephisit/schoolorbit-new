<script lang="ts">
	import { type Snippet } from 'svelte';
	import { Upload, X, Image as ImageIcon } from 'lucide-svelte';
	import { Button } from '$lib/components/ui/button';
	import { cn } from '$lib/utils';

	interface Props {
		value?: string | null;
		maxSizeMB?: number;
		accept?: string;
		disabled?: boolean;
		className?: string;
		previewSize?: 'sm' | 'md' | 'lg';
		onupload?: (file: File) => void;
		onremove?: () => void;
		onerror?: (error: string) => void;
		helper?: Snippet;
	}

	let {
		value = null,
		maxSizeMB = 5,
		accept = 'image/jpeg,image/png,image/webp,image/gif',
		disabled = false,
		className = '',
		previewSize = 'md',
		onupload,
		onremove,
		onerror,
		helper
	}: Props = $props();

	// State
	let isDragging = $state(false);
	let error = $state<string | null>(null);
	let uploading = $state(false);
	let preview = $state<string | null>(value);
	let fileInput = $state<HTMLInputElement>();

	// Preview size classes
	const sizeClasses = {
		sm: 'w-24 h-24',
		md: 'w-32 h-32',
		lg: 'w-48 h-48'
	};

	// Update preview when value changes
	$effect(() => {
		if (value !== preview) {
			preview = value;
		}
	});

	// Handle file selection
	function handleFileSelect(event: Event) {
		const target = event.target as HTMLInputElement;
		const file = target.files?.[0];
		if (file) {
			processFile(file);
		}
	}

	// Handle drag and drop
	function handleDrop(event: DragEvent) {
		event.preventDefault();
		isDragging = false;

		const file = event.dataTransfer?.files[0];
		if (file) {
			processFile(file);
		}
	}

	function handleDragOver(event: DragEvent) {
		event.preventDefault();
		isDragging = true;
	}

	function handleDragLeave() {
		isDragging = false;
	}

	// Process selected file
	function processFile(file: File) {
		error = null;

		// Validate file type
		if (!file.type.startsWith('image/')) {
			error = 'กรุณาเลือกไฟล์รูปภาพเท่านั้น';
			onerror?.(error);
			return;
		}

		// Validate file size
		const fileSizeMB = file.size / (1024 * 1024);
		if (fileSizeMB > maxSizeMB) {
			error = `ขนาดไฟล์ใหญ่เกิน ${maxSizeMB} MB`;
			onerror?.(error);
			return;
		}

		// Create preview
		const reader = new FileReader();
		reader.onload = (e) => {
			preview = e.target?.result as string;
		};
		reader.readAsDataURL(file);

		// Dispatch upload event
		onupload?.(file);
	}

	// Remove image
	function handleRemove() {
		preview = null;
		error = null;
		if (fileInput) {
			fileInput.value = '';
		}
		onremove?.();
	}

	// Trigger file input
	function triggerFileInput() {
		fileInput?.click();
	}
</script>

<div class={cn('space-y-4', className)}>
	<!-- Preview or Upload Area -->
	<div
		class={cn(
			'relative rounded-lg border-2 border-dashed transition-colors',
			sizeClasses[previewSize],
			isDragging ? 'border-primary bg-primary/5' : 'border-muted-foreground/25',
			disabled && 'opacity-50 cursor-not-allowed'
		)}
		ondrop={handleDrop}
		ondragover={handleDragOver}
		ondragleave={handleDragLeave}
		role="button"
		tabindex={disabled ? -1 : 0}
		onclick={triggerFileInput}
		onkeydown={(e) => e.key === 'Enter' && triggerFileInput()}
	>
		{#if preview}
			<!-- Image Preview -->
			<div class="relative w-full h-full group">
				<img src={preview} alt="Preview" class="w-full h-full object-cover rounded-lg" />

				<!-- Remove Button -->
				{#if !disabled}
					<button
						type="button"
						class="absolute top-2 right-2 p-1 bg-destructive text-destructive-foreground rounded-full opacity-0 group-hover:opacity-100 transition-opacity"
						onclick={(e) => {
							e.stopPropagation();
							handleRemove();
						}}
						aria-label="ลบรูปภาพ"
					>
						<X class="w-4 h-4" />
					</button>
				{/if}
			</div>
		{:else}
			<!-- Upload Prompt -->
			<div class="flex flex-col items-center justify-center w-full h-full p-4 text-center">
				{#if uploading}
					<div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary"></div>
					<p class="mt-2 text-sm text-muted-foreground">กำลังอัปโหลด...</p>
				{:else}
					<ImageIcon class="w-8 h-8 text-muted-foreground mb-2" />
					<p class="text-sm text-muted-foreground">คลิกหรือลากรูปภาพมาที่นี่</p>
					<p class="text-xs text-muted-foreground mt-1">
						ขนาดไม่เกิน {maxSizeMB} MB
					</p>
				{/if}
			</div>
		{/if}

		<!-- Hidden File Input -->
		<input
			bind:this={fileInput}
			type="file"
			{accept}
			{disabled}
			onchange={handleFileSelect}
			class="hidden"
			aria-label="เลือกรูปภาพ"
		/>
	</div>

	<!-- Upload Button (Alternative) -->
	{#if !preview && !uploading}
		<Button
			type="button"
			variant="outline"
			size="sm"
			onclick={triggerFileInput}
			{disabled}
			class="w-full"
		>
			<Upload class="w-4 h-4 mr-2" />
			เลือกรูปภาพ
		</Button>
	{/if}

	<!-- Error Message -->
	{#if error}
		<p class="text-sm text-destructive">{error}</p>
	{/if}

	<!-- Helper Text -->
	{#if helper}
		{@render helper()}
	{:else}
		<p class="text-xs text-muted-foreground">
			รองรับ: JPG, PNG, WebP, GIF (สูงสุด {maxSizeMB} MB)
		</p>
	{/if}
</div>
