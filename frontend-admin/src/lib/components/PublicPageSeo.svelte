<script lang="ts">
	import { serializeStructuredData, type PublicSite } from '#lib/public-site.js';
	let {
		title,
		description,
		site,
		structuredData
	}: {
		title: string;
		description: string;
		site: PublicSite;
		structuredData?: Record<string, unknown>;
	} = $props();
</script>

<svelte:head>
	<title>{title}</title>
	<meta name="description" content={description} />
	<link rel="canonical" href={site.canonicalUrl} />
	<meta name="robots" content={site.indexable ? 'index, follow' : 'noindex, follow'} />
	<meta property="og:type" content="website" />
	<meta property="og:locale" content="th_TH" />
	<meta property="og:site_name" content="SchoolOrbit" />
	<meta property="og:url" content={site.canonicalUrl} />
	<meta property="og:title" content={title} />
	<meta property="og:description" content={description} />
	<meta name="twitter:card" content="summary" />
	<meta name="twitter:title" content={title} />
	<meta name="twitter:description" content={description} />
	{#if structuredData}
		<svelte:element this={"script"} type="application/ld+json"
			>{serializeStructuredData(structuredData)}</svelte:element
		>
	{/if}
</svelte:head>
