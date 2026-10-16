<script lang="ts">
	import { publicFileUrl } from '#lib/api/files.js';
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
	<img src="/schoolorbit.png" alt="SchoolOrbit" width="96" height="96" class={className} />
{/if}
