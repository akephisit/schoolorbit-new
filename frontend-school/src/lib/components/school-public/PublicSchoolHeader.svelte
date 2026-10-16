<script lang="ts">
	import { resolve } from '$app/paths';
	import type { PublicSchoolInfo } from '#lib/api/school.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import PublicSchoolBrand from './PublicSchoolBrand.svelte';
	import { ArrowUpRight, Menu, X } from '@lucide/svelte';
	let {
		operation,
		current = 'home'
	}: {
		operation: RouteLoadResult<PublicSchoolInfo> | Promise<RouteLoadResult<PublicSchoolInfo>>;
		current?: 'home' | 'executives' | 'management' | 'subjects';
	} = $props();
	let menuOpen = $state(false);
	const links = $derived([
		{ label: 'คณะผู้บริหาร', href: resolve('/executives'), active: current === 'executives' },
		{ label: 'กลุ่มบริหาร', href: resolve('/management-groups'), active: current === 'management' },
		{ label: 'กลุ่มสาระ', href: resolve('/subject-groups'), active: current === 'subjects' }
	]);
</script>

<header class="sticky top-0 z-30 border-b border-border/70 bg-background/95 backdrop-blur-xl">
	<div class="mx-auto flex max-w-6xl flex-wrap items-center gap-2 px-4 py-3 sm:gap-3 sm:px-6">
		<a
			href={resolve('/')}
			class="flex min-w-0 flex-1 items-center gap-2.5 font-medium"
			data-testid="school-brand"
		>
			<PublicSchoolBrand {operation} />
		</a>
		<nav
			id="public-school-navigation"
			aria-label="เมนูโรงเรียน"
			class={`${menuOpen ? 'flex' : 'hidden'} order-last w-full flex-col gap-1 border-t border-border pt-2 lg:order-none lg:flex lg:w-auto lg:flex-row lg:border-0 lg:pt-0`}
		>
			{#each links as link (link.href)}
				<Button
					href={link.href}
					variant={link.active ? 'secondary' : 'ghost'}
					aria-current={link.active ? 'page' : undefined}
					class="justify-start lg:justify-center"
					onclick={() => (menuOpen = false)}>{link.label}</Button
				>
			{/each}
		</nav>
		<Button href={resolve('/login')}
			>เข้าสู่ระบบ <ArrowUpRight class="hidden size-4 sm:block" /></Button
		>
		<Button
			variant="ghost"
			size="icon"
			class="lg:hidden"
			aria-label={menuOpen ? 'ปิดเมนูโรงเรียน' : 'เปิดเมนูโรงเรียน'}
			aria-expanded={menuOpen}
			aria-controls="public-school-navigation"
			onclick={() => (menuOpen = !menuOpen)}
		>
			{#if menuOpen}<X class="size-5" />{:else}<Menu class="size-5" />{/if}
		</Button>
	</div>
</header>
