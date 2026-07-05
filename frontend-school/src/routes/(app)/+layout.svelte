<script lang="ts">
	import Sidebar from '$lib/components/layout/Sidebar.svelte';
	import Header from '$lib/components/layout/Header.svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { authAPI } from '$lib/api/auth';
	import { userCanAccessRoute } from '$lib/auth/route-access';
	import { authStore } from '$lib/stores/auth';
	import { userPermissions } from '$lib/stores/permissions';

	import { uiPreferences } from '$lib/stores/ui-preferences';
	import { notificationStore } from '$lib/stores/notification';
	let { children } = $props();

	type AuthStatus = 'checking' | 'authenticated' | 'redirecting';

	let sidebarRef = $state<{ toggleMobileSidebar?: () => void }>();
	let isSidebarCollapsed = $state($uiPreferences.sidebarCollapsed);
	let authStatus = $state<AuthStatus>('checking');

	function handleMenuClick() {
		if (sidebarRef?.toggleMobileSidebar) {
			sidebarRef.toggleMobileSidebar();
		}
	}

	function currentPath() {
		return `${window.location.pathname}${window.location.search}${window.location.hash}`;
	}

	async function redirectToLogin(rememberCurrentPath = false) {
		if (rememberCurrentPath) {
			sessionStorage.setItem('redirectAfterLogin', currentPath());
		} else {
			sessionStorage.removeItem('redirectAfterLogin');
		}

		authStatus = 'redirecting';
		await goto(resolve('/login'), { replaceState: true });
	}

	async function redirectToForbidden() {
		authStatus = 'redirecting';
		await goto(resolve(`/403?from=${encodeURIComponent(currentPath())}`), {
			replaceState: true
		});
		authStatus = 'authenticated';
	}

	function canAccessCurrentRoute() {
		return userCanAccessRoute($authStore.user, $userPermissions, page.route.id);
	}

	// Check authentication for protected routes
	onMount(async () => {
		const isAuthenticated = await authAPI.checkAuth();

		if (!isAuthenticated) {
			await redirectToLogin(true);
			return;
		}

		if (!canAccessCurrentRoute()) {
			await redirectToForbidden();
			return;
		}

		authStatus = 'authenticated';
		notificationStore.syncExistingPushSubscription();
	});

	$effect(() => {
		const routeId = page.route.id;
		const permissions = $userPermissions;
		const user = $authStore.user;

		if (authStatus !== 'authenticated') return;
		if (!user) {
			void redirectToLogin();
			return;
		}
		if (userCanAccessRoute(user, permissions, routeId)) return;

		void redirectToForbidden();
	});
</script>

{#if authStatus === 'authenticated'}
	<div class="h-screen flex flex-col bg-background overflow-hidden">
		<Sidebar bind:this={sidebarRef} bind:isCollapsed={isSidebarCollapsed} />

		<!-- Wrapper for Header and Main with sidebar offset -->
		<div
			class="flex flex-col flex-1 min-h-0 transition-[margin-left] duration-300 {isSidebarCollapsed
				? 'lg:ml-[72px]'
				: 'lg:ml-64'}"
		>
			<!-- Fixed Header - ไม่ scroll -->
			<Header onMenuClick={handleMenuClick} sidebarCollapsed={isSidebarCollapsed} />

			<!-- Main Content - scroll อยู่ที่นี่ -->
			<main class="flex-1 min-h-0 overflow-y-auto">
				<div class="h-full">
					{@render children()}
				</div>
			</main>
		</div>
	</div>
{:else}
	<div class="h-screen bg-background flex items-center justify-center" aria-live="polite">
		<div class="flex flex-col items-center gap-4 text-muted-foreground">
			<div class="h-10 w-10 rounded-full border-2 border-muted border-t-primary animate-spin"></div>
			<p>{authStatus === 'redirecting' ? 'กำลังเปลี่ยนหน้า...' : 'กำลังตรวจสอบสิทธิ์...'}</p>
		</div>
	</div>
{/if}
