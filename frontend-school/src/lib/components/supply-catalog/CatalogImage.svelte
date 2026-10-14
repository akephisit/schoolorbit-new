<script lang="ts">
	import PrivateFileImage from '#lib/components/files/PrivateFileImage.svelte';
	import { Package } from '@lucide/svelte';
	let {
		fileId,
		resourceId,
		alt,
		class: className = ''
	}: {
		fileId: string | null | undefined;
		resourceId?: string;
		alt: string;
		class?: string;
	} = $props();
</script>

<div class={`relative flex items-center justify-center overflow-hidden bg-muted/40 ${className}`}>
	<Package
		class="size-10 text-muted-foreground/30"
		aria-label={fileId ? 'รูปพัสดุ' : 'ไม่มีรูปพัสดุ'}
	/>
	{#if fileId}<PrivateFileImage
			{fileId}
			{resourceId}
			{alt}
			class="absolute inset-0 h-full w-full object-contain"
		/>{/if}
</div>
