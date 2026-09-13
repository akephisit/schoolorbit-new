<script lang="ts">
	import PromotionRunDetail from '$lib/components/academic/lifecycle/PromotionRunDetail.svelte';
	import type { PageData } from './$types';
	let { data }: { data: PageData } = $props();
</script>

{#key data.runId}<PromotionRunDetail runId={data.runId} />{/key}
