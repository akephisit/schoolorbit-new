<script lang="ts">
	import Sidebar from '$lib/components/layout/Sidebar.svelte';
	import Header from '$lib/components/layout/Header.svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { authAPI } from '$lib/api/auth';
	import type { AuthRefreshResult } from '$lib/auth/auth-refresh-policy';
	import { userCanAccessRoute } from '$lib/auth/route-access';
	import { authStore } from '$lib/stores/auth';
	import { userPermissions } from '$lib/stores/permissions';
	import { AuthCheckingState, PageState } from '$lib/components/app-state';
	import { toast } from 'svelte-sonner';

	import { uiPreferences } from '$lib/stores/ui-preferences';
	import { notificationStore } from '$lib/stores/notification';
	let { children } = $props();

	type AuthStatus = 'checking' | 'authenticated' | 'unavailable' | 'redirecting';

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

	async function applyAuthenticationResult(
		result: AuthRefreshResult,
		rememberCurrentPath: boolean
	) {
		if (result === 'unauthenticated') {
			await redirectToLogin(rememberCurrentPath);
			return;
		}

		if (result === 'unavailable') {
			if ($authStore.isAuthenticated) {
				authStatus = 'authenticated';
				toast.warning('ระบบยืนยันตัวตนไม่พร้อมใช้งาน กรุณาลองใหม่อีกครั้ง');
			} else {
				authStatus = 'unavailable';
			}
			return;
		}

		if (!canAccessCurrentRoute()) {
			await redirectToForbidden();
			return;
		}

		authStatus = 'authenticated';
		notificationStore.syncExistingPushSubscription();
	}

	async function authenticate(rememberCurrentPath: boolean) {
		const result = await authAPI.refreshCurrentUser({ silent: false });
		await applyAuthenticationResult(result, rememberCurrentPath);
	}

	async function retryAuthentication() {
		authStatus = 'checking';
		await authenticate(true);
	}

	onMount(async () => {
		await authenticate(true);
	});

	$effect(() => {
		const routeId = page.route.id;
		const permissions = $userPermissions;
		const user = $authStore.user;

		if (authStatus !== 'authenticated') return;
		if (!user) {
			void redirectToLogin(true);
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
{:else if authStatus === 'unavailable'}
	<div class="flex min-h-screen items-center justify-center bg-background p-4">
		<PageState
			variant="error"
			title="ระบบยืนยันตัวตนไม่พร้อมใช้งาน"
			description="ระบบยังตรวจสอบสถานะการเข้าสู่ระบบไม่ได้ กรุณาลองใหม่อีกครั้ง"
			actionLabel="ลองอีกครั้ง"
			onaction={retryAuthentication}
			class="w-full max-w-lg"
		/>
	</div>
{:else}
	<AuthCheckingState
		message={authStatus === 'redirecting' ? 'กำลังเปลี่ยนหน้า...' : 'กำลังตรวจสอบสิทธิ์...'}
	/>
{/if}
