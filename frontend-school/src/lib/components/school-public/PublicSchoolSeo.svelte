<script lang="ts">
	import type { PublicSchoolInfo } from '#lib/api/school.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import {
		buildSchoolHomeSeo,
		type SchoolHomeSeo,
		type SchoolPublicIndexing
	} from '#lib/school-public/seo.js';

	let {
		operation,
		site
	}: {
		operation: RouteLoadResult<PublicSchoolInfo> | Promise<RouteLoadResult<PublicSchoolInfo>>;
		site: SchoolPublicIndexing;
	} = $props();
	function metadata(info: PublicSchoolInfo | null): SchoolHomeSeo {
		return buildSchoolHomeSeo(info, site, new URL('/favicon.png', site.homeUrl).href);
	}
</script>

<svelte:head>
	<link rel="canonical" href={site.homeUrl} />
	<meta name="robots" content={site.indexable ? 'index, follow' : 'noindex, follow'} />
	{#await operation}
		{@render tags(metadata(null))}
	{:then result}
		{@render tags(metadata(result.ok ? result.data : null))}
	{/await}
</svelte:head>

{#snippet tags(seo: SchoolHomeSeo)}
	<title>{seo.title}</title>
	<meta name="description" content={seo.description} />
	<meta property="og:type" content="website" />
	<meta property="og:locale" content="th_TH" />
	<meta property="og:site_name" content={seo.siteName} />
	<meta property="og:url" content={site.homeUrl} />
	<meta property="og:title" content={seo.title} />
	<meta property="og:description" content={seo.description} />
	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:title" content={seo.title} />
	<meta name="twitter:description" content={seo.description} />
	{#if seo.image}
		<meta property="og:image" content={seo.image} />
		<meta property="og:image:width" content="1200" />
		<meta property="og:image:height" content="630" />
		<meta property="og:image:type" content="image/png" />
		<meta property="og:image:alt" content={seo.imageAlt} />
		<meta name="twitter:image" content={seo.image} />
		<meta name="twitter:image:alt" content={seo.imageAlt} />
	{/if}
	{#if seo.structuredData}
		<svelte:element this={"script"} type="application/ld+json">{seo.structuredData}</svelte:element>
	{/if}
{/snippet}
