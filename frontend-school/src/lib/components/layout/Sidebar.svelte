<script lang="ts">
	import { LogOut, ChevronLeft, GraduationCap } from 'lucide-svelte';
	import { resolve } from '$app/paths';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { authAPI } from '$lib/api/auth';
	import { authStore } from '$lib/stores/auth';
	import {
		userPermissions,
		permissionsLoading,
		loadUserPermissions
	} from '$lib/stores/permissions';
	import {
		menuItems,
		filterMenusByPermission,
		getMenusByGroup,
		type MenuItem
	} from '$lib/config/menu-permissions';

	let { isCollapsed = $bindable(false) }: { isCollapsed?: boolean } = $props();
	let isMobileOpen = $state(false);

	// Reactive filtered menus based on permissions
	let filteredMenus = $derived(filterMenusByPermission(menuItems, $userPermissions));
	let mainMenus = $derived(getMenusByGroup(filteredMenus, 'main'));
	let adminMenus = $derived(getMenusByGroup(filteredMenus, 'admin'));
	let settingsMenus = $derived(getMenusByGroup(filteredMenus, 'settings'));

	// Load permissions reactively when user changes
	$effect(() => {
		const user = $authStore.user;
		if (user?.id) {
			loadUserPermissions(user.id);
		}
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

	function renderMenuItem(item: MenuItem, isActiveMenu: boolean) {
		const Icon = item.icon;
		return { Icon, isActive: isActiveMenu };
	}
</script>

<!-- Mobile Overlay -->
{#if isMobileOpen}
	<div
		class="fixed inset-0 bg-black/50 z-40 lg:hidden"
		onclick={toggleMobileSidebar}
		role="button"
		tabindex="0"
		onkeydown={(e) => e.key === 'Enter' && toggleMobileSidebar()}
	></div>
{/if}

<!-- Sidebar -->
<aside
	class="fixed left-0 top-0 z-50 h-screen bg-card border-r border-border transition-all duration-300 ease-in-out
		{isCollapsed ? 'w-20' : 'w-72'}
		{isMobileOpen ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'}"
>
	<div class="flex flex-col h-full">
		<!-- Header -->
		<div class="h-16 border-b border-border flex items-center relative px-6">
			{#if !isCollapsed}
				<!-- Expanded State - Icon box absolute positioned -->
				<div
					class="absolute left-5 w-10 h-10 bg-primary rounded-lg flex items-center justify-center"
				>
					<GraduationCap class="w-6 h-6 text-primary-foreground" />
				</div>
				<div
					class="ml-[60px] min-w-0 flex-1 overflow-hidden transition-opacity duration-300 {isCollapsed
						? 'opacity-0'
						: 'opacity-100'}"
				>
					<h2 class="font-bold text-foreground text-lg whitespace-nowrap">SchoolOrbit</h2>
					<p class="text-xs text-muted-foreground whitespace-nowrap">ระบบจัดการโรงเรียน</p>
				</div>

				<!-- Toggle Button - Always visible when expanded -->
				<button
					onclick={toggleSidebar}
					class="hidden lg:flex w-6 h-6 items-center justify-center rounded hover:bg-accent transition-colors flex-shrink-0 ml-auto"
					aria-label="Toggle Sidebar"
				>
					<ChevronLeft class="w-4 h-4 text-muted-foreground" />
				</button>
			{:else}
				<!-- Collapsed State - Icon box absolute positioned (same position) -->
				<button
					onclick={toggleSidebar}
					class="hidden lg:block absolute left-5 w-10 h-10 bg-primary rounded-lg hover:bg-primary/90 p-0 border-0 transition-colors"
					aria-label="Expand Sidebar"
				>
					<div class="w-full h-full flex items-center justify-center relative group/icon">
						<!-- Icon - visible by default -->
						<GraduationCap
							class="w-6 h-6 text-primary-foreground absolute transition-all duration-200 group-hover/icon:opacity-0 group-hover/icon:scale-75"
						/>
						<!-- Arrow - visible on hover -->
						<ChevronLeft
							class="w-5 h-5 text-primary-foreground absolute transition-all duration-200 opacity-0 scale-75 rotate-180 group-hover/icon:opacity-100 group-hover/icon:scale-100"
						/>
					</div>
				</button>
			{/if}
		</div>

		<!-- Navigation -->
		<nav class="flex-1 overflow-y-auto p-4 space-y-1">
			{#if $permissionsLoading}
				<!-- Loading skeleton -->
				<div class="space-y-2">
					{#each Array(6) as _}
						<div class="h-10 bg-muted rounded-lg animate-pulse"></div>
					{/each}
				</div>
			{:else if filteredMenus.length === 0}
				<!-- No permissions -->
				{#if !isCollapsed}
					<div class="p-4 text-center">
						<p class="text-sm text-muted-foreground">ไม่มีเมนูที่สามารถเข้าถึงได้</p>
						<p class="text-xs text-muted-foreground mt-1">กรุณาติดต่อผู้ดูแลระบบ</p>
					</div>
				{/if}
			{:else}
				<!-- Main Navigation -->
				{#each mainMenus as item (item.href)}
					{@const { Icon } = renderMenuItem(item, isActive(item.href))}
					<a
						href={resolve(item.href as any)}
						onclick={handleNavClick}
						class="relative flex items-center px-3 py-2.5 rounded-lg transition-colors group
							{isActive(item.href)
							? 'bg-primary text-primary-foreground'
							: 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'}"
					>
						<Icon
							class="absolute left-[14px] w-5 h-5 {isActive(item.href)
								? 'text-primary-foreground'
								: 'text-muted-foreground group-hover:text-accent-foreground'}"
						/>
						<span
							class="ml-[50px] font-medium whitespace-nowrap overflow-hidden transition-opacity duration-300 {isCollapsed
								? 'opacity-0'
								: 'opacity-100'}">{item.name}</span
						>
					</a>
				{/each}

				<!-- Admin Section -->
				{#if adminMenus.length > 0}
					<div class="pt-4">
						{#if !isCollapsed}
							<div class="px-3 py-2">
								<p class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
									ผู้ดูแลระบบ
								</p>
							</div>
						{:else}
							<div class="border-t border-border my-2"></div>
						{/if}

						{#each adminMenus as item (item.href)}
							{@const { Icon } = renderMenuItem(item, isActive(item.href))}
							<a
								href={resolve(item.href as any)}
								onclick={handleNavClick}
								class="relative flex items-center px-3 py-2.5 rounded-lg transition-colors group
									{isActive(item.href)
									? 'bg-purple-500 text-white'
									: 'text-muted-foreground hover:bg-purple-50 hover:text-purple-700'}"
							>
								<Icon
									class="absolute left-[14px] w-5 h-5 {isActive(item.href)
										? 'text-white'
										: 'text-muted-foreground group-hover:text-purple-700'}"
								/>
								<span
									class="ml-[50px] font-medium whitespace-nowrap overflow-hidden transition-opacity duration-300 {isCollapsed
										? 'opacity-0'
										: 'opacity-100'}">{item.name}</span
								>
							</a>
						{/each}
					</div>
				{/if}
			{/if}
		</nav>

		<!-- Bottom Navigation -->
		<div class="border-t border-border p-4 space-y-1">
			<!-- Settings -->
			{#each settingsMenus as item (item.href)}
				{@const { Icon } = renderMenuItem(item, isActive(item.href))}
				<a
					href={resolve(item.href as any)}
					class="flex items-center gap-3 px-3 py-2.5 rounded-lg transition-colors
						{isCollapsed ? 'justify-center' : ''}
						text-muted-foreground hover:bg-accent hover:text-accent-foreground group"
				>
					<Icon
						class="absolute left-[14px] w-5 h-5 text-muted-foreground group-hover:text-accent-foreground"
					/>
					<span
						class="ml-[50px] font-medium whitespace-nowrap overflow-hidden transition-opacity duration-300 {isCollapsed
							? 'opacity-0'
							: 'opacity-100'}">{item.name}</span
					>
				</a>
			{/each}

			<!-- Logout Button -->
			<button
				onclick={handleLogout}
				class="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg transition-colors
					text-muted-foreground hover:bg-destructive/10 hover:text-destructive group"
			>
				<LogOut class="w-5 h-5 flex-shrink-0 text-muted-foreground group-hover:text-destructive" />
				<span
					class="ml-[50px] font-medium whitespace-nowrap overflow-hidden transition-opacity duration-300 {isCollapsed
						? 'opacity-0'
						: 'opacity-100'}">ออกจากระบบ</span
				>
			</button>
		</div>
	</div>
</aside>
