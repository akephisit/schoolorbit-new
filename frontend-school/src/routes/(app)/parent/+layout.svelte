<script lang="ts">
	import type { Snippet } from 'svelte';

	let { children }: { children?: Snippet } = $props();
</script>

<div class="min-h-screen bg-background">
	<!-- Parent Portal Content -->
	{@render children?.()}
</div>
