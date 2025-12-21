<script lang="ts">
	import Sidebar from '$lib/components/layout/Sidebar.svelte';
	import Header from '$lib/components/layout/Header.svelte';

	let { children } = $props();

	let sidebarRef: { toggleMobileSidebar?: () => void } | undefined;
	let isSidebarCollapsed = $state(false);

	function handleMenuClick() {
		if (sidebarRef?.toggleMobileSidebar) {
			sidebarRef.toggleMobileSidebar();
		}
	}
</script>

<div class="h-screen flex flex-col bg-background overflow-hidden">
	<Sidebar bind:this={sidebarRef} bind:isCollapsed={isSidebarCollapsed} />

	<!-- Wrapper for Header and Main with sidebar offset -->
	<div
		class="flex flex-col flex-1 min-h-0 transition-all duration-300 {isSidebarCollapsed
			? 'lg:ml-20'
			: 'lg:ml-72'}"
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
