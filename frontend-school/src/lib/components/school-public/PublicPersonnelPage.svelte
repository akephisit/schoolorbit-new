<script lang="ts">
	import type { PublicSchoolInfo, PublicSchoolOrganization } from '#lib/api/school.js';
	import { getRequiredPublicSchoolInfo, getPublicSchoolOrganization } from '#lib/api/school.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import type { SchoolPublicIndexing } from '#lib/school-public/seo.js';
	import { PageSkeleton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import PublicSchoolHeader from './PublicSchoolHeader.svelte';
	import PublicSchoolRegion from './PublicSchoolRegion.svelte';
	import PublicPersonnelDirectory from './PublicPersonnelDirectory.svelte';
	let {
		data,
		unitType
	}: {
		data: {
			site: SchoolPublicIndexing;
			identity: RouteLoadResult<PublicSchoolInfo>;
			organization:
				| RouteLoadResult<PublicSchoolOrganization>
				| Promise<RouteLoadResult<PublicSchoolOrganization>>;
		};
		unitType: 'management_group' | 'subject_group';
	} = $props();
	let identityRetry = $state<Promise<RouteLoadResult<PublicSchoolInfo>> | null>(null);
	const operation = $derived(identityRetry ?? data.identity);
	const title = $derived(unitType === 'management_group' ? 'กลุ่มบริหาร' : 'กลุ่มสาระ');
	const description = $derived(
		unitType === 'management_group'
			? 'ผู้รับผิดชอบ หัวหน้า และสมาชิก แยกตามกลุ่มบริหารและฝ่ายงาน'
			: 'หัวหน้าและสมาชิก แยกตามกลุ่มสาระการเรียนรู้'
	);
	const path = $derived(unitType === 'management_group' ? '/management-groups' : '/subject-groups');
</script>

<svelte:head>
	{#await operation}
		<title>{title} — เว็บไซต์โรงเรียน</title>
	{:then result}
		<title
			>{title} — {result.ok
				? result.data.schoolName || 'เว็บไซต์โรงเรียน'
				: 'เว็บไซต์โรงเรียน'}</title
		>
	{/await}
	<meta name="description" content={description} />
	<meta name="robots" content="noindex, follow" />
	<link rel="canonical" href={new URL(path, data.site.homeUrl).href} />
</svelte:head>

<div class="min-h-screen bg-background text-foreground">
	<a
		href="#main-content"
		class="sr-only focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-50 focus:rounded-lg focus:bg-card focus:p-3"
		>ข้ามไปเนื้อหาหลัก</a
	>
	<PublicSchoolHeader
		{operation}
		current={unitType === 'management_group' ? 'management' : 'subjects'}
	/>
	<main id="main-content" tabindex="-1" class="mx-auto max-w-6xl px-4 py-8 sm:px-6 sm:py-12">
		<Button href="/" variant="link" class="mb-4 px-0">กลับหน้าหลัก</Button>
		<div class="mb-7">
			<PublicSchoolRegion
				source={data.identity}
				bind:retryResult={identityRetry}
				label="ข้อมูลโรงเรียน"
				retry={(signal) => getRequiredPublicSchoolInfo({ signal })}
			>
				{#snippet skeleton()}<PageSkeleton variant="cards" rows={1} />{/snippet}
				{#snippet children(info)}<p class="text-sm text-primary">
						{info.schoolName || 'บุคลากรของโรงเรียน'}
					</p>{/snippet}
			</PublicSchoolRegion>
			<h1 class="mt-2 text-2xl font-medium sm:text-3xl">{title}</h1>
			<p class="mt-3 text-sm leading-relaxed text-muted-foreground">{description}</p>
		</div>
		<PublicSchoolRegion
			source={data.organization}
			label="รายชื่อบุคลากร"
			retry={(signal) => getPublicSchoolOrganization({ signal })}
		>
			{#snippet skeleton()}<PageSkeleton variant="cards" rows={4} />{/snippet}
			{#snippet children(organization)}<PublicPersonnelDirectory
					{organization}
					{unitType}
				/>{/snippet}
		</PublicSchoolRegion>
	</main>
</div>
