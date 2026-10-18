<script lang="ts">
	import type { PageProps } from './$types';
	import StaffDesignPreview from '#lib/components/appearance/StaffDesignPreview.svelte';
	let { data }: PageProps = $props();
</script>

<svelte:head><title>ตัวอย่างหน้าหลักบุคลากร - SchoolOrbit</title></svelte:head>
<div style="container-type: inline-size">
	<StaffDesignPreview appearance={data.appearance} mode={data.mode === 'dark' ? 'dark' : 'light'} />
</div>
