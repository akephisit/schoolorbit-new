<script lang="ts">
	import type { PublicSchoolInfo } from '#lib/api/school.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { publicFileUrl } from '#lib/api/files.js';
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
		return buildSchoolHomeSeo(info, site, info?.logoFileId ? publicFileUrl(info.logoFileId) : null);
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
	<meta name="twitter:card" content="summary" />
	<meta name="twitter:title" content={seo.title} />
	<meta name="twitter:description" content={seo.description} />
	{#if seo.image}
		<meta property="og:image" content={seo.image} />
		<meta name="twitter:image" content={seo.image} />
	{/if}
	{#if seo.structuredData}
		<svelte:element this={"script"} type="application/ld+json">{seo.structuredData}</svelte:element>
	{/if}
{/snippet}
