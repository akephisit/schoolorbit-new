<script lang="ts">
	import type { PublicSchoolInfo, PublicSchoolOrganization } from '#lib/api/school.js';
	import { getRequiredPublicSchoolInfo, getPublicSchoolOrganization } from '#lib/api/school.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import type { SchoolPublicIndexing } from '#lib/school-public/seo.js';
	import { PageSkeleton } from '#lib/components/app-state/index.js';
	import PublicSchoolHeader from './PublicSchoolHeader.svelte';
	import PublicSchoolRegion from './PublicSchoolRegion.svelte';
	import PublicExecutiveTeam from './PublicExecutiveTeam.svelte';
	import PublicPersonnelDirectory from './PublicPersonnelDirectory.svelte';
	let {
		data,
		view
	}: {
		data: {
			site: SchoolPublicIndexing;
			identity: RouteLoadResult<PublicSchoolInfo>;
			organization:
				| RouteLoadResult<PublicSchoolOrganization>
				| Promise<RouteLoadResult<PublicSchoolOrganization>>;
		};
		view: 'executives' | 'management_group' | 'subject_group';
	} = $props();
	let identityRetry = $state<Promise<RouteLoadResult<PublicSchoolInfo>> | null>(null);
	const operation = $derived(identityRetry ?? data.identity);
	const title = $derived(
		view === 'executives'
			? 'คณะผู้บริหาร'
			: view === 'management_group'
				? 'กลุ่มบริหาร'
				: 'กลุ่มสาระ'
	);
	const description = $derived(
		view === 'executives'
			? 'ผู้อำนวยการ รองผู้อำนวยการ และผู้รับผิดชอบกลุ่มบริหาร'
			: view === 'management_group'
				? 'ผู้รับผิดชอบ หัวหน้า และสมาชิก แยกตามกลุ่มบริหารและฝ่ายงาน'
				: 'หัวหน้าและสมาชิก แยกตามกลุ่มสาระการเรียนรู้'
	);
	const path = $derived(
		view === 'executives'
			? '/executives'
			: view === 'management_group'
				? '/management-groups'
				: '/subject-groups'
	);
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

<div class="min-h-screen bg-muted/20 text-foreground">
	<a
		href="#main-content"
		class="sr-only focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-50 focus:rounded-lg focus:bg-card focus:p-3"
		>ข้ามไปเนื้อหาหลัก</a
	>
	<PublicSchoolHeader
		{operation}
		current={view === 'executives'
			? 'executives'
			: view === 'management_group'
				? 'management'
				: 'subjects'}
	/>
	<main
		id="main-content"
		tabindex="-1"
		class="mx-auto max-w-6xl px-4 pt-7 pb-10 sm:px-6 sm:pt-10 sm:pb-14"
	>
		<header class="mb-6 text-center sm:mb-8">
			<div class="mx-auto mb-3 h-1 w-10 rounded-full bg-primary" aria-hidden="true"></div>
			<h1 class="text-2xl font-medium tracking-tight sm:text-3xl">{title}</h1>
			<p class="mt-2 text-sm leading-relaxed text-muted-foreground">{description}</p>
		</header>
		<PublicSchoolRegion
			source={data.identity}
			bind:retryResult={identityRetry}
			label="ข้อมูลโรงเรียน"
			retry={(signal) => getRequiredPublicSchoolInfo({ signal })}
		>
			{#snippet skeleton()}<PageSkeleton variant="cards" rows={1} />{/snippet}
		</PublicSchoolRegion>
		<PublicSchoolRegion
			source={data.organization}
			label="รายชื่อบุคลากร"
			retry={(signal) => getPublicSchoolOrganization({ signal })}
		>
			{#snippet skeleton()}<PageSkeleton variant="cards" rows={4} />{/snippet}
			{#snippet children(organization)}
				{#if view === 'executives'}
					<PublicExecutiveTeam {organization} />
				{:else}
					<PublicPersonnelDirectory {organization} unitType={view} />
				{/if}
			{/snippet}
		</PublicSchoolRegion>
	</main>
</div>
