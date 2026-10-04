<script lang="ts">
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	import SupervisionWorkspace from '#lib/components/supervision/SupervisionWorkspace.svelte';
</script>

<SupervisionWorkspace section="templates" routeData={data} />
