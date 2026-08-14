<script lang="ts">
	import PublicCertificateVerification from '$lib/components/certificates/PublicCertificateVerification.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<svelte:head>
	<title>{data.title}</title>
	<meta name="description" content={data.description} />
	<meta name="referrer" content="no-referrer" />
	<meta name="robots" content="noindex, nofollow" />
</svelte:head>

<PublicCertificateVerification />
