<script lang="ts">
	import { LogOut, ChevronLeft, GraduationCap } from 'lucide-svelte';
	import { resolve } from '$app/paths';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { authAPI } from '$lib/api/auth';
	import { authStore } from '$lib/stores/auth';
	import { getUserMenu, type MenuGroup, type MenuItem } from '$lib/api/menu';
	import { getIconComponent } from '$lib/utils/icon-mapper';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { uiPreferences } from '$lib/stores/ui-preferences';

	let { isCollapsed = $bindable($uiPreferences.sidebarCollapsed) }: { isCollapsed?: boolean } = $props();
	let isMobileOpen = $state(false);

	// Dynamic menu state
	let menuGroups = $state<MenuGroup[]>([]);
	let menuLoading = $state(true);

	// Load menu from API
	async function loadMenu() {
		try {
			menuLoading = true;
			const response = await getUserMenu();
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

	// Check if a route is active
	function isActive(href: string): boolean {
		return page.url.pathname.startsWith(href);
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

	async function handleLogout() {
		await authAPI.logout();
		await goto('/login', { invalidateAll: true });
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
  {isCollapsed ? 'w-[80px]' : 'w-64'}
  {isMobileOpen ? 'translate-x-0' : '-translate-x-full'}
  lg:translate-x-0 flex flex-col"
>
	<!-- Header -->
	<div class="flex items-center justify-between p-4 h-16">
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
		<nav class="flex-1 overflow-y-auto p-4 space-y-1">
			{#if menuLoading}
				<!-- Loading skeleton -->
				<div class="space-y-2">
					{#each Array(6) as _}
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
				{#each menuGroups as group (group.code)}
					{#each group.items as item (item.id)}
						{@const Icon = getIconComponent(item.icon)}
						<Tooltip.Root delayDuration={0} disabled={!isCollapsed}>
							<Tooltip.Trigger class="w-full">
								<a
									href={resolve(item.path as any)}
									onclick={handleNavClick}
									class="relative flex items-center px-3 py-2.5 rounded-lg transition-colors group
									{isActive(item.path)
										? 'bg-primary text-primary-foreground'
										: 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'}"
								>
									<Icon
										class="absolute left-[14px] w-5 h-5 {isActive(item.path)
											? 'text-primary-foreground'
											: 'text-muted-foreground group-hover:text-accent-foreground'}"
									/>
									<span
										class="ml-[50px] font-medium whitespace-nowrap overflow-hidden transition-opacity duration-300 {isCollapsed
											? 'opacity-0'
											: 'opacity-100'}">{item.name}</span
									>
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

	<!-- Bottom Navigation -->
	<Tooltip.Provider>
		<div class="border-t border-border p-4 space-y-1">
			<!-- Logout -->
			<Tooltip.Root delayDuration={0} disabled={!isCollapsed}>
				<Tooltip.Trigger class="w-full">
					<div
						role="button"
						tabindex="0"
						onclick={handleLogout}
						onkeydown={(e) => {
							if (e.key === 'Enter' || e.key === ' ') handleLogout();
						}}
						class="relative flex items-center px-3 py-2.5 rounded-lg transition-colors
              text-muted-foreground hover:bg-accent hover:text-accent-foreground group cursor-pointer"
					>
						<LogOut
							class="absolute left-[14px] w-5 h-5 text-muted-foreground group-hover:text-accent-foreground"
						/>
						<span
							class="ml-[50px] font-medium whitespace-nowrap overflow-hidden transition-opacity duration-300 {isCollapsed
								? 'opacity-0'
								: 'opacity-100'}">ออกจากระบบ</span
						>
					</div>
				</Tooltip.Trigger>
				{#if isCollapsed}
					<Tooltip.Content side="right" class="font-medium">ออกจากระบบ</Tooltip.Content>
				{/if}
			</Tooltip.Root>
		</div>
	</Tooltip.Provider>
</aside>

<style>
	/* Remove default button styles for logout div */
	div[role='button'] {
		-webkit-tap-highlight-color: transparent;
	}
</style>
