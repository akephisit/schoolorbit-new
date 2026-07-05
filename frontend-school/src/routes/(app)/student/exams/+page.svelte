<script lang="ts">
	import type { PageProps } from './$types';
	import { PageShell } from '$lib/components/app-layout';

	let { data }: PageProps = $props();
</script>

<svelte:head>
	<title>{data.title}</title>
</svelte:head>

<PageShell title={data.title} />
