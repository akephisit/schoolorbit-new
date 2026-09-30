<script lang="ts">
	import SupervisionWorkspace from '$lib/components/supervision/SupervisionWorkspace.svelte';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
</script>

<SupervisionWorkspace section="requests" routeData={data} />
