<script lang="ts">
	import { ChevronLeft, GraduationCap } from 'lucide-svelte';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { authStore } from '$lib/stores/auth';
	import { getUserMenu, type MenuGroup, type MenuItem } from '$lib/api/menu';
	import { getIconComponent } from '$lib/utils/icon-mapper';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { uiPreferences } from '$lib/stores/ui-preferences';

	let { isCollapsed = $bindable($uiPreferences.sidebarCollapsed) }: { isCollapsed?: boolean } =
		$props();
	let isMobileOpen = $state(false);

	// Dynamic menu state
	let menuGroups = $state<MenuGroup[]>([]);
	let menuLoading = $state(true);

	// Load menu from API
	async function loadMenu() {
		try {
			menuLoading = true;
			const response = await getUserMenu();

			// Backend already filters by user_type - use response directly
			menuGroups = response.groups;
		} catch (error) {
			console.error('Failed to load menu:', error);
			menuGroups = [];
		} finally {
			menuLoading = false;
		}
	}

	// Load menu when user is authenticated
	$effect(() => {
		const user = $authStore.user;
		if (user?.id) {
			loadMenu();
		}
	});

	// Sync isCollapsed changes to localStorage
	$effect(() => {
		uiPreferences.setSidebarCollapsed(isCollapsed);
	});

	// Get all menu paths for finding best match
	let allMenuPaths = $derived.by(() => {
		const paths: string[] = [];
		for (const group of menuGroups) {
			for (const item of group.items) {
				paths.push(item.path);
			}
		}
		return paths;
	});

	// Check if a route is active
	// Only highlights the BEST matching menu item (longest matching path)
	function isActive(href: string): boolean {
		const currentPath = page.url.pathname;

		// Exact match
		if (currentPath === href) {
			return true;
		}

		// Child route match - but only if no other menu has a better match
		if (currentPath.startsWith(href + '/')) {
			// Find if there's a better (longer) match
			const betterMatch = allMenuPaths.find(
				(menuPath) =>
					menuPath !== href &&
					menuPath.length > href.length &&
					(currentPath === menuPath || currentPath.startsWith(menuPath + '/'))
			);

			// Only highlight if this is the best match
			return !betterMatch;
		}

		return false;
	}

	// Handle navigation click on mobile
	function handleNavClick() {
		if (isMobileOpen) {
			isMobileOpen = false;
		}
	}

	function toggleSidebar() {
		isCollapsed = !isCollapsed;
	}

	export function toggleMobileSidebar() {
		isMobileOpen = !isMobileOpen;
	}
</script>

<!-- Mobile Overlay -->
{#if isMobileOpen}
	<div
		class="fixed inset-0 bg-black/50 z-40 lg:hidden"
		onclick={toggleMobileSidebar}
		onkeydown={(e) => {
			if (e.key === 'Enter' || e.key === ' ' || e.key === 'Escape') {
				toggleMobileSidebar();
			}
		}}
		role="button"
		tabindex="0"
		aria-label="Close sidebar"
	></div>
{/if}

<!-- Sidebar Container -->
<aside
	class="fixed left-0 top-0 z-50 h-screen bg-card border-r border-border transition-all duration-300 ease-in-out
  {isCollapsed ? 'w-[72px]' : 'w-64'}
  {isMobileOpen ? 'translate-x-0' : '-translate-x-full'}
  lg:translate-x-0 flex flex-col"
>
	<!-- Header -->
	<div class="flex items-center justify-between p-4 border-b border-border h-16">
		{#if !isCollapsed}
			<div class="flex items-center gap-2 overflow-hidden">
				<GraduationCap class="w-8 h-8 text-primary flex-shrink-0" />
				<h1 class="text-lg font-bold text-foreground whitespace-nowrap">SchoolOrbit</h1>
			</div>
		{/if}

		<!-- Toggle Button (Desktop) -->
		<button
			onclick={toggleSidebar}
			class="hidden lg:flex p-2 rounded-lg hover:bg-accent transition-colors
      {isCollapsed ? 'mx-auto' : ''}"
			aria-label={isCollapsed ? 'Expand sidebar' : 'Collapse sidebar'}
		>
			<div class="transition-transform duration-300 {isCollapsed ? 'rotate-180' : ''}">
				<ChevronLeft class="w-5 h-5" />
			</div>
		</button>
	</div>

	<!-- Navigation -->
	<Tooltip.Provider>
		<nav class="flex-1 overflow-y-auto overflow-x-hidden py-4 space-y-1 sidebar-nav px-4">
			{#if menuLoading}
				<!-- Loading skeleton -->
				<div class="space-y-2">
					{#each Array(6) as _, idx (idx)}
						<div class="h-10 bg-muted rounded-lg animate-pulse"></div>
					{/each}
				</div>
			{:else if menuGroups.length === 0}
				<!-- No menus -->
				{#if !isCollapsed}
					<div class="p-4 text-center">
						<p class="text-sm text-muted-foreground">ไม่มีเมนูที่สามารถเข้าถึงได้</p>
						<p class="text-xs text-muted-foreground mt-1">กรุณาติดต่อผู้ดูแลระบบ</p>
					</div>
				{/if}
			{:else}
				<!-- Dynamic Menu Groups -->
				{#each menuGroups as group, groupIndex (group.code)}
					<!-- Group Header (except first group) -->
					{#if groupIndex > 0}
						<div class="relative my-3 h-5 flex items-center px-3">
							<!-- Divider line (visible when collapsed) -->
							<div
								class="flex-1 border-t border-border transition-opacity duration-300
								{isCollapsed ? 'opacity-100' : 'opacity-0'}"
							></div>
							<!-- Group name (visible when expanded) -->
							<p
								class="absolute text-xs font-semibold text-muted-foreground uppercase tracking-wider whitespace-nowrap transition-opacity duration-300
								{isCollapsed ? 'opacity-0' : 'opacity-100'}"
							>
								{group.name}
							</p>
						</div>
					{/if}

					{#each group.items as item (item.id)}
						{@const Icon = getIconComponent(item.icon)}
						<Tooltip.Root delayDuration={0} disabled={!isCollapsed}>
							<Tooltip.Trigger class="w-full">
								<a
									href={resolve(item.path as any)}
									onclick={handleNavClick}
									class="relative flex items-center h-[40px] rounded-lg transition-all duration-300 group px-0 {isCollapsed
										? 'w-[40px]'
										: 'w-full'} {isActive(item.path)
										? 'bg-primary text-primary-foreground'
										: 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'}"
								>
									<div class="w-[40px] h-[40px] flex items-center justify-center flex-shrink-0">
										<Icon
											class="w-5 h-5 transition-colors {isActive(item.path)
												? 'text-primary-foreground'
												: 'text-muted-foreground group-hover:text-accent-foreground'}"
										/>
									</div>
									<span
										class="font-medium whitespace-nowrap overflow-hidden transition-all duration-300 {isCollapsed
											? 'w-0 opacity-0 hidden'
											: 'w-auto opacity-100 ml-1'}"
									>
										{item.name}
									</span>
								</a>
							</Tooltip.Trigger>
							{#if isCollapsed}
								<Tooltip.Content side="right" class="font-medium">
									{item.name}
								</Tooltip.Content>
							{/if}
						</Tooltip.Root>
					{/each}
				{/each}
			{/if}
		</nav>
	</Tooltip.Provider>
</aside>

<style>
	/* Modern minimal scrollbar with gradient and glow */
	.sidebar-nav {
		/* Smooth scrolling */
		scroll-behavior: smooth;
		
		/* Firefox - minimal style */
		scrollbar-width: thin;
		scrollbar-color: transparent transparent;
	}

	/* Show gradient scrollbar on hover (Firefox) */
	.sidebar-nav:hover {
		scrollbar-color: oklch(0.5 0.02 240 / 0.3) transparent;
	}

	/* Webkit browsers (Chrome, Safari, Edge) */
	.sidebar-nav::-webkit-scrollbar {
		width: 2px;
	}

	.sidebar-nav::-webkit-scrollbar-track {
		background: transparent;
	}

	/* Hide scrollbar arrows/buttons */
	.sidebar-nav::-webkit-scrollbar-button {
		display: none;
		height: 0;
		width: 0;
	}

	/* Scrollbar thumb - hidden by default with fade transition */
	.sidebar-nav::-webkit-scrollbar-thumb {
		background: transparent;
		border-radius: 10px;
		transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
		opacity: 0;
	}

	/* Show scrollbar with gray gradient and glow on hover */
	.sidebar-nav:hover::-webkit-scrollbar-thumb {
		background: linear-gradient(
			180deg,
			oklch(0.5 0.02 240 / 0.4) 0%,
			oklch(0.45 0.02 240 / 0.5) 50%,
			oklch(0.5 0.02 240 / 0.4) 100%
		);
		box-shadow: 
			0 0 8px oklch(0.5 0.02 240 / 0.25),
			0 0 4px oklch(0.5 0.02 240 / 0.4);
		opacity: 1;
	}

	/* Enhanced glow when hovering directly on scrollbar */
	.sidebar-nav:hover::-webkit-scrollbar-thumb:hover {
		background: linear-gradient(
			180deg,
			oklch(0.55 0.02 240 / 0.6) 0%,
			oklch(0.5 0.02 240 / 0.7) 50%,
			oklch(0.55 0.02 240 / 0.6) 100%
		);
		box-shadow: 
			0 0 12px oklch(0.5 0.02 240 / 0.4),
			0 0 6px oklch(0.5 0.02 240 / 0.6),
			0 0 2px oklch(0.5 0.02 240 / 0.8);
	}

	/* Dark mode adjustments */
	:global(.dark) .sidebar-nav:hover::-webkit-scrollbar-thumb {
		background: linear-gradient(
			180deg,
			oklch(0.65 0.02 240 / 0.5) 0%,
			oklch(0.7 0.02 240 / 0.6) 50%,
			oklch(0.65 0.02 240 / 0.5) 100%
		);
		box-shadow: 
			0 0 10px oklch(0.65 0.02 240 / 0.35),
			0 0 5px oklch(0.65 0.02 240 / 0.5);
	}

	:global(.dark) .sidebar-nav:hover::-webkit-scrollbar-thumb:hover {
		background: linear-gradient(
			180deg,
			oklch(0.7 0.02 240 / 0.7) 0%,
			oklch(0.75 0.02 240 / 0.8) 50%,
			oklch(0.7 0.02 240 / 0.7) 100%
		);
		box-shadow: 
			0 0 15px oklch(0.65 0.02 240 / 0.5),
			0 0 8px oklch(0.65 0.02 240 / 0.7),
			0 0 3px oklch(0.7 0.02 240 / 0.9);
	}
</style>
