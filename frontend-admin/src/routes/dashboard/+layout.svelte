<script lang="ts">
	import { authStore } from '#lib/stores/auth.svelte.js';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { Orbit, LogOut, School, ClipboardList } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	let { children }: { children: Snippet } = $props();
	let ready = $state(false);
	let operator = $derived(authStore.user?.role === 'operator');
	let allowed = $derived(
		operator ||
			(!page.url.pathname.startsWith('/dashboard/schools') &&
				!page.url.pathname.startsWith('/dashboard/migrations'))
	);
	onMount(() => {
		let active = true;
		void authStore.ready.then(() => {
			if (!active) return;
			ready = true;
			if (!authStore.isAuthenticated) void goto(resolve('/login'));
		});
		return () => {
			active = false;
		};
	});
</script>

<svelte:head><meta name="robots" content="noindex" /></svelte:head>
<div class="portal-app">
	<header class="portal-header">
		<a class="portal-brand" href={resolve('/')}><Orbit size={28} /> SchoolOrbit</a>
		<nav aria-label="เมนูหลัก">
			<a
				href={resolve('/dashboard')}
				aria-current={page.url.pathname === '/dashboard' ? 'page' : undefined}
				><School size={18} /> โรงเรียนของฉัน</a
			>
			<a
				href={resolve('/dashboard/requests')}
				aria-current={page.url.pathname === '/dashboard/requests' ? 'page' : undefined}
				><ClipboardList size={18} /> {operator ? 'คำขอจากลูกค้า' : 'คำขอรับบริการ'}</a
			>
			{#if operator}<a href={resolve('/dashboard/schools')}>จัดการโรงเรียน</a><a
					href={resolve('/dashboard/migrations')}>Migration</a
				>{/if}
		</nav>
		<button class="portal-button secondary" onclick={() => authStore.logout()}
			><LogOut size={16} /> ออกจากระบบ</button
		>
	</header>
	<main class="portal-main">
		{#if !ready}<p role="status">กำลังโหลดบัญชี…</p>
		{:else if !authStore.isAuthenticated}<p>กรุณาเข้าสู่ระบบ</p>
		{:else if !allowed}<div class="portal-error" role="alert">
				หน้านี้สำหรับทีม SchoolOrbit เท่านั้น
			</div>
		{:else}<div class="portal-account">
				<span>{authStore.user?.name}</span><span>{authStore.user?.email}</span>
			</div>
			{@render children()}{/if}
		{#if authStore.error}<p class="portal-error" role="alert">{authStore.error}</p>{/if}
	</main>
</div>
