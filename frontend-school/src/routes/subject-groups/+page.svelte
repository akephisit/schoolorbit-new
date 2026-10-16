<script lang="ts">
	import PublicPersonnelPage from '#lib/components/school-public/PublicPersonnelPage.svelte';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
</script>

<PublicPersonnelPage {data} view="subject_group" />
