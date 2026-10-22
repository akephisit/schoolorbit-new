<script lang="ts">
	import { resolve } from '$app/paths';
	import { ArrowRight, ArrowUpRight, GraduationCap, Orbit } from '@lucide/svelte';
	import PublicPageSeo from '#lib/components/PublicPageSeo.svelte';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	const schools = $derived(
		data.directory.schools.map((school) => ({
			...school,
			url: `https://${school.subdomain}.${data.publicSite.baseDomain}/`
		}))
	);
</script>

<PublicPageSeo
	title="เว็บไซต์โรงเรียน — SchoolOrbit"
	description="ค้นหาเว็บไซต์โรงเรียนที่เปิดให้บริการบน SchoolOrbit พร้อมลิงก์ข้อมูลและบริการสาธารณะของแต่ละโรงเรียน"
	site={data.publicSite}
	structuredData={{
		'@context': 'https://schema.org',
		'@type': 'CollectionPage',
		name: 'เว็บไซต์โรงเรียนบน SchoolOrbit',
		url: data.publicSite.canonicalUrl,
		mainEntity: {
			'@type': 'ItemList',
			itemListElement: schools.map((school, index) => ({
				'@type': 'ListItem',
				position: index + 1,
				name: school.name,
				url: school.url
			}))
		}
	}}
/>
<div class="min-h-screen bg-[#f8fbfc] text-[#163441]">
	<header class="mx-auto flex max-w-5xl items-center justify-between gap-4 px-6 py-8">
		<a href={resolve('/')} class="flex items-center gap-2 text-xl font-semibold text-[#237c9b]"
			><Orbit aria-hidden="true" /> SchoolOrbit</a
		>
		<a href={resolve('login')} class="text-sm text-[#237c9b] hover:underline"
			>เข้าสู่ระบบบัญชีบริการ</a
		>
	</header>
	<main class="mx-auto max-w-5xl px-6 py-12">
		<p class="text-sm text-[#237c9b]">ข้อมูลและบริการสาธารณะ</p>
		<h1 class="mt-3 text-3xl font-semibold sm:text-4xl">เว็บไซต์โรงเรียน</h1>
		<p class="mt-5 max-w-2xl leading-relaxed text-[#54717e]">
			เลือกโรงเรียนเพื่อดูข้อมูลและบริการสาธารณะของโรงเรียน
		</p>
		{#if schools.length}
			<ul class="mt-10 grid gap-5 sm:grid-cols-2">
				{#each schools as school (school.subdomain)}
					<li>
						<a
							href={school.url}
							class="flex h-full items-center gap-4 rounded-3xl border border-[#dce9ee] bg-white p-6 hover:border-[#237c9b] focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-[#237c9b]"
						>
							<GraduationCap class="size-7 shrink-0 text-[#237c9b]" aria-hidden="true" />
							<span class="min-w-0 flex-1"
								><span class="block text-lg font-medium break-words">{school.name}</span><span
									class="mt-1 block text-sm break-all text-[#54717e]"
									>{school.subdomain}.{data.publicSite.baseDomain}</span
								></span
							>
							<ArrowUpRight class="size-5 shrink-0 text-[#237c9b]" aria-hidden="true" />
						</a>
					</li>
				{/each}
			</ul>
		{:else}<p class="mt-10 rounded-3xl border border-[#dce9ee] bg-white p-8 text-[#54717e]">
				ยังไม่มีรายชื่อโรงเรียนในหน้านี้
			</p>{/if}
		<nav aria-label="หน้ารายชื่อโรงเรียน" class="mt-10 flex flex-wrap gap-6 text-[#237c9b]">
			{#if data.publicSite.canonicalUrl.includes('?')}<a
					href={resolve('schools')}
					class="hover:underline">กลับหน้าแรกของรายชื่อ</a
				>{/if}
			{#if data.directory.nextCursor}<a
					href={`${resolve('schools')}?after=${encodeURIComponent(data.directory.nextCursor)}`}
					class="inline-flex items-center gap-2 hover:underline"
					>โรงเรียนถัดไป <ArrowRight class="size-4" aria-hidden="true" /></a
				>{/if}
		</nav>
	</main>
</div>
