<script lang="ts">
	import type { PageProps } from './$types';
	import SupervisionWorkspace from '#lib/components/supervision/SupervisionWorkspace.svelte';

	let { data }: PageProps = $props();
</script>

<SupervisionWorkspace section="mine" routeData={data} />
