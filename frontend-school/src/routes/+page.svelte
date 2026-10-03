<script lang="ts">
	import type { PageProps } from './$types';
	import { resolve } from '$app/paths';
	import { Button } from '$lib/components/ui/button';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import {
		getRequiredPublicSchoolInfo,
		getPublicSchoolStatistics,
		getPublicSchoolOrganization
	} from '$lib/api/school';
	import { buildPublicOrganizationTree } from '$lib/school-public/organization';
	import PublicSchoolIdentity from '$lib/components/school-public/PublicSchoolIdentity.svelte';
	import PublicSchoolRegion from '$lib/components/school-public/PublicSchoolRegion.svelte';
	import PublicSchoolStatistics from '$lib/components/school-public/PublicSchoolStatistics.svelte';
	import PublicOrganizationTree from '$lib/components/school-public/PublicOrganizationTree.svelte';
	import {
		ArrowDown,
		ArrowUpRight,
		BadgeCheck,
		CalendarDays,
		GraduationCap,
		Menu,
		X,
		School,
		Orbit
	} from '@lucide/svelte';
	let { data }: PageProps = $props();
	let menuOpen = $state(false);
	const services = [
		{
			title: 'ปฏิทินโรงเรียน',
			description: 'ติดตามกิจกรรมและกำหนดการที่โรงเรียนเปิดเผยให้ทุกคนดูได้',
			href: resolve('/calendar'),
			icon: CalendarDays,
			label: 'ดูปฏิทิน'
		},
		{
			title: 'รับสมัครนักเรียน',
			description: 'ดูรอบการรับสมัคร สมัครเรียน และติดตามสถานะการสมัคร',
			href: resolve('/apply'),
			icon: GraduationCap,
			label: 'ดูการรับสมัคร'
		},
		{
			title: 'ตรวจสอบเกียรติบัตร',
			description: 'ตรวจสอบความถูกต้องของเกียรติบัตรที่ออกโดยโรงเรียน',
			href: resolve('/verify/certificate'),
			icon: BadgeCheck,
			label: 'ตรวจสอบเอกสาร'
		}
	];
</script>

<svelte:head>
	<title>เว็บไซต์โรงเรียน — ข้อมูลและบริการสาธารณะ</title>
	<meta
		name="description"
		content="รู้จักโรงเรียน ดูสถิตินักเรียน ครู ห้องเรียน และโครงสร้างบริหาร พร้อมเข้าถึงบริการสาธารณะ"
	/>
</svelte:head>
<div class="school-public min-h-screen bg-background text-foreground">
	<a href="#main-content" class="skip-link rounded-lg bg-card px-4 py-3 text-primary shadow-lg"
		>ข้ามไปเนื้อหาหลัก</a
	>
	<header class="sticky top-0 z-30 border-b border-border bg-background/95 backdrop-blur">
		<div
			class="relative mx-auto flex max-w-6xl items-center justify-between gap-3 px-4 py-3 sm:px-6"
		>
			<a href={resolve('/')} class="flex min-w-0 items-center gap-2.5 font-medium"
				><span
					class="flex size-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary"
					><School class="size-6" /></span
				><span class="text-sm sm:text-base">เว็บไซต์โรงเรียน</span></a
			>
			<nav
				id="school-navigation"
				aria-label="เมนูหลัก"
				class="school-navigation"
				class:mobile-open={menuOpen}
			>
				<a href="#statistics" onclick={() => (menuOpen = false)}>ข้อมูลโรงเรียน</a>
				<a href="#organization" onclick={() => (menuOpen = false)}>โครงสร้างบริหาร</a>
				<a href="#services" onclick={() => (menuOpen = false)}>บริการ</a>
			</nav>
			<div class="flex shrink-0 items-center gap-1.5">
				<Button href="/login">เข้าสู่ระบบ <ArrowUpRight class="hidden size-4 sm:block" /></Button>
				<Button
					variant="ghost"
					size="icon"
					class="md:hidden"
					aria-label={menuOpen ? 'ปิดเมนู' : 'เปิดเมนู'}
					aria-expanded={menuOpen}
					aria-controls="school-navigation"
					onclick={() => (menuOpen = !menuOpen)}
					>{#if menuOpen}<X class="size-5" />{:else}<Menu class="size-5" />{/if}</Button
				>
			</div>
		</div>
	</header>
	<main id="main-content" tabindex="-1">
		<section
			class="relative isolate overflow-hidden border-b border-border bg-gradient-to-br from-primary/5 via-background to-accent px-4 py-14 sm:px-6 sm:py-20"
		>
			<Orbit
				class="pointer-events-none absolute -right-24 -bottom-24 -z-10 size-96 text-primary/5"
				strokeWidth={0.7}
				aria-hidden="true"
			/>
			<div class="mx-auto max-w-6xl">
				<p class="mb-6 text-xs font-medium tracking-widest text-primary">
					ข้อมูลและบริการสาธารณะของโรงเรียน
				</p>
				<PublicSchoolRegion
					source={data.identity}
					label="ข้อมูลโรงเรียน"
					retry={(signal) => getRequiredPublicSchoolInfo({ signal })}
				>
					{#snippet skeleton()}<div class="space-y-4">
							<Skeleton class="h-16 w-16 rounded-2xl" /><Skeleton
								class="h-10 w-3/4 max-w-lg"
							/><Skeleton class="h-5 w-1/2" />
						</div>{/snippet}
					{#snippet children(info)}
						<PublicSchoolIdentity {info} />
					{/snippet}
				</PublicSchoolRegion>
				<div class="mt-8 flex flex-wrap gap-3">
					<Button href="/#statistics" size="lg"
						>ดูข้อมูลโรงเรียน <ArrowDown class="size-4" /></Button
					><Button href="/#services" size="lg" variant="outline"
						>สำรวจบริการ <ArrowUpRight class="size-4" /></Button
					>
				</div>
			</div>
		</section>
		<section
			id="statistics"
			aria-labelledby="statistics-heading"
			class="section-anchor mx-auto max-w-6xl px-4 py-12 sm:px-6 sm:py-16"
		>
			<div class="mb-6">
				<p class="text-xs font-medium tracking-widest text-primary">ภาพรวมโรงเรียน</p>
				<h2 id="statistics-heading" class="mt-3 text-2xl font-medium sm:text-3xl">
					โรงเรียนของเราในวันนี้
				</h2>
				<p class="mt-3 text-sm leading-relaxed text-muted-foreground">
					ข้อมูลนักเรียน ห้องเรียน และบุคลากรจากระบบของโรงเรียน
				</p>
			</div>
			<PublicSchoolRegion
				source={data.statistics}
				label="สถิติโรงเรียน"
				retry={(signal) => getPublicSchoolStatistics({ signal })}
			>
				{#snippet skeleton()}<PageSkeleton variant="cards" rows={4} />
					<div class="mt-8"><PageSkeleton variant="table" rows={3} columns={5} /></div>{/snippet}
				{#snippet children(statistics)}<PublicSchoolStatistics {statistics} />{/snippet}
			</PublicSchoolRegion>
		</section>
		<section
			id="organization"
			aria-labelledby="organization-heading"
			class="section-anchor border-y border-border bg-muted/30 px-4 py-12 sm:px-6 sm:py-16"
		>
			<div class="mx-auto max-w-6xl">
				<div class="mb-6">
					<p class="text-xs font-medium tracking-widest text-primary">โครงสร้างองค์กร</p>
					<h2 id="organization-heading" class="mt-3 text-2xl font-medium sm:text-3xl">
						แผนผังการบริหาร
					</h2>
					<p class="mt-3 text-sm leading-relaxed text-muted-foreground">
						หน่วยงานและผู้ดำรงตำแหน่งปัจจุบัน · กดชื่อหน่วยงานเพื่อกางหรือย่อข้อมูล
					</p>
				</div>
				<PublicSchoolRegion
					source={data.organization}
					label="โครงสร้างบริหาร"
					retry={(signal) => getPublicSchoolOrganization({ signal })}
				>
					{#snippet skeleton()}<PageSkeleton variant="cards" rows={3} />{/snippet}
					{#snippet children(organization)}{#if organization.units.length}<PublicOrganizationTree
								nodes={buildPublicOrganizationTree(organization.units)}
							/>{:else}<PageState
								title="ยังไม่มีข้อมูลโครงสร้างบริหาร"
								description="ข้อมูลจะแสดงเมื่อโรงเรียนตั้งค่าโครงสร้างองค์กร"
							/>{/if}{/snippet}
				</PublicSchoolRegion>
			</div>
		</section>
		<section
			id="services"
			aria-labelledby="services-heading"
			class="section-anchor mx-auto max-w-6xl px-4 py-12 sm:px-6 sm:py-16"
		>
			<p class="text-xs font-medium tracking-widest text-primary">บริการของโรงเรียน</p>
			<h2 id="services-heading" class="mt-3 text-2xl font-medium sm:text-3xl">
				เรื่องของโรงเรียน เริ่มต้นได้ที่นี่
			</h2>
			<div class="mt-7 grid gap-4 md:grid-cols-3">
				{#each services as service (service.href)}
					<a
						href={service.href}
						class="group flex flex-col rounded-2xl border border-border bg-card p-6 transition-colors hover:border-primary/40 hover:bg-accent/40"
						><span
							class="flex size-12 items-center justify-center rounded-xl bg-primary/10 text-primary"
							><service.icon class="size-6" strokeWidth={1.5} /></span
						>
						<h3 class="mt-5 text-xl font-medium">{service.title}</h3>
						<p class="mt-3 flex-1 text-sm leading-relaxed text-muted-foreground">
							{service.description}
						</p>
						<span
							class="mt-6 flex items-center justify-between border-t border-border pt-4 text-sm font-medium text-primary"
							>{service.label}<ArrowUpRight class="size-4" /></span
						></a
					>
				{/each}
			</div>
		</section>
	</main>
	<footer class="border-t border-border">
		<div
			class="mx-auto flex max-w-6xl flex-wrap items-center justify-between gap-4 px-4 py-7 text-sm text-muted-foreground sm:px-6"
		>
			<span class="flex items-center gap-2"
				><Orbit class="size-4" /> ขับเคลื่อนด้วย SchoolOrbit</span
			><a href={resolve('/privacy-policy')} class="hover:text-primary">นโยบายความเป็นส่วนตัว</a>
		</div>
	</footer>
</div>

<style>
	.school-public :global(a:focus-visible),
	.school-public :global(button:focus-visible),
	.school-public :global(summary:focus-visible) {
		outline: 2px solid var(--ring);
		outline-offset: 4px;
	}
	.skip-link {
		position: fixed;
		top: 1rem;
		left: 1rem;
		z-index: 50;
		transform: translateY(-200%);
	}
	.skip-link:focus {
		transform: translateY(0);
	}
	.section-anchor {
		scroll-margin-top: 6rem;
	}
	.school-navigation {
		display: flex;
		align-items: center;
		gap: 1.5rem;
		font-size: 0.875rem;
		color: var(--muted-foreground);
	}
	.school-navigation a:hover {
		color: var(--primary);
	}
	@media (max-width: 767px) {
		.school-navigation {
			display: none;
			position: absolute;
			top: calc(100% + 0.5rem);
			left: 1rem;
			right: 1rem;
			border: 1px solid var(--border);
			border-radius: 1rem;
			padding: 0.5rem;
			background: var(--card);
		}
		.school-navigation.mobile-open {
			display: flex;
			align-items: stretch;
			flex-direction: column;
			gap: 0;
		}
		.school-navigation a {
			padding: 0.875rem 1rem;
			border-radius: 0.5rem;
		}
		.school-navigation a:hover {
			background: var(--accent);
		}
	}
</style>
