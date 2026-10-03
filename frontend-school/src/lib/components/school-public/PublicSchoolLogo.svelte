<script lang="ts">
	import { publicFileUrl } from '$lib/api/files';
	import { School } from '@lucide/svelte';
	let { fileId, class: className = '' }: { fileId?: string | null; class?: string } = $props();
	let failedLogo = $state<string | null>(null);
</script>

{#if fileId && failedLogo !== fileId}
	<img
		src={publicFileUrl(fileId)}
		alt="ตราโรงเรียน"
		width="96"
		height="120"
		class={className}
		onerror={() => (failedLogo = fileId ?? null)}
	/>
{:else}
	<School class={`${className} text-primary`} strokeWidth={1.5} aria-hidden="true" />
{/if}
