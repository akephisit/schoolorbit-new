<script lang="ts">
	import Sidebar from '$lib/components/layout/Sidebar.svelte';
	import Header from '$lib/components/layout/Header.svelte';

	let { children } = $props();

	let sidebarRef: any;
	let isSidebarCollapsed = $state(false);

	function handleMenuClick() {
		if (sidebarRef?.toggleMobileSidebar) {
			sidebarRef.toggleMobileSidebar();
		}
	}
</script>

<div class="min-h-screen bg-background">
	<Sidebar bind:this={sidebarRef} />
	<Header onMenuClick={handleMenuClick} sidebarCollapsed={isSidebarCollapsed} />

	<!-- Main Content -->
	<main
		class="transition-all duration-300 pt-16
		{isSidebarCollapsed ? 'lg:pl-20' : 'lg:pl-72'}"
	>
		<div class="p-4 lg:p-6">
			{@render children()}
		</div>
	</main>
</div>
