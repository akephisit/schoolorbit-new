<script lang="ts">
	import Sidebar from '$lib/components/layout/Sidebar.svelte';
	import Header from '$lib/components/layout/Header.svelte';
	import { onMount } from 'svelte';
	import { authAPI } from '$lib/api/auth';

	import { uiPreferences } from '$lib/stores/ui-preferences';
	let { children } = $props();

	let sidebarRef: { toggleMobileSidebar?: () => void } | undefined;
	let isSidebarCollapsed = $state($uiPreferences.sidebarCollapsed);

	function handleMenuClick() {
		if (sidebarRef?.toggleMobileSidebar) {
			sidebarRef.toggleMobileSidebar();
		}
	}

	// Check authentication for protected routes
	onMount(async () => {
		await authAPI.checkAuth();
	});
</script>

<div class="h-screen flex flex-col bg-background overflow-hidden">
	<Sidebar bind:this={sidebarRef} bind:isCollapsed={isSidebarCollapsed} />

	<!-- Wrapper for Header and Main with sidebar offset -->
	<div
		class="flex flex-col flex-1 min-h-0 transition-all duration-300 {isSidebarCollapsed
			? 'lg:ml-[80px]'
			: 'lg:ml-64'}"
	>
		<!-- Fixed Header - ไม่ scroll -->
		<Header onMenuClick={handleMenuClick} sidebarCollapsed={isSidebarCollapsed} />

		<!-- Main Content - scroll อยู่ที่นี่ -->
		<main class="flex-1 min-h-0 overflow-y-auto">
			<div class="p-4 lg:p-6">
				{@render children()}
			</div>
		</main>
	</div>
</div>
