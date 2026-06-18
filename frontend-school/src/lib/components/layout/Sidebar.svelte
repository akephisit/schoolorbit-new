<script lang="ts">
	import { ChevronDown, ChevronLeft, GraduationCap, Inbox } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { authStore } from '$lib/stores/auth';
	import { getUserMenu, type MenuGroup } from '$lib/api/menu';
	import { Button, buttonVariants } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { uiPreferences } from '$lib/stores/ui-preferences';
	import { workStore } from '$lib/stores/work';
	import { cn } from '$lib/utils';
	import { getIconComponent } from '$lib/utils/icon-mapper';
	import {
		buildSidebarNavigation,
		type SidebarMenuItem,
		type SidebarMenuSection,
		type SidebarWorkspaceSection
	} from './sidebar-navigation';

	let { isCollapsed = $bindable($uiPreferences.sidebarCollapsed) }: { isCollapsed?: boolean } =
		$props();
	let isMobileOpen = $state(false);
	let menuGroups = $state<MenuGroup[]>([]);
	let menuLoading = $state(true);

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

	$effect(() => {
		const user = $authStore.user;
		if (user?.id) {
			loadMenu();
			void workStore.fetchCounts({ silent: true });
		} else {
			menuGroups = [];
			workStore.reset();
		}
	});

	$effect(() => {
		uiPreferences.setSidebarCollapsed(isCollapsed);
	});

	let workspaceSections = $derived.by(() => buildSidebarNavigation(menuGroups));

	let allMenuPaths = $derived.by(() => {
		const paths: string[] = ['/staff/work'];
		for (const workspace of workspaceSections) {
			for (const section of workspace.sections) {
				for (const item of section.items) {
					paths.push(item.path);
				}
			}
		}
		return paths;
	});

	let activeWorkCount = $derived(
		$workStore.counts.open + $workStore.counts.dueSoon + $workStore.counts.overdue
	);
	let urgentWorkCount = $derived($workStore.counts.dueSoon + $workStore.counts.overdue);

	function sectionHasActiveItem(section: SidebarMenuSection): boolean {
		return section.items.some((item) => isActive(item.path));
	}

	function workspaceHasActiveItem(workspace: SidebarWorkspaceSection): boolean {
		return workspace.sections.some(sectionHasActiveItem);
	}

	function sectionExpanded(section: SidebarMenuSection): boolean {
		if (isCollapsed) return false;

		const savedValue = $uiPreferences.sidebarExpandedGroups[section.id];
		if (savedValue !== undefined) return savedValue;

		return sectionHasActiveItem(section) || section.defaultOpen;
	}

	function toggleSection(section: SidebarMenuSection) {
		uiPreferences.setSidebarGroupExpanded(section.id, !sectionExpanded(section));
	}

	function navItemClass(item: SidebarMenuItem, nested = false): string {
		return cn(
			'relative h-9 w-full justify-start gap-2 rounded-md px-2 text-sm transition-all',
			nested && 'pl-3',
			isActive(item.path)
				? 'bg-primary text-primary-foreground hover:bg-primary/90 hover:text-primary-foreground'
				: 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
		);
	}

	function sectionTriggerClass(section: SidebarMenuSection): string {
		return cn(
			'h-9 w-full justify-start gap-2 rounded-md px-2 text-xs font-semibold text-muted-foreground hover:bg-accent hover:text-accent-foreground',
			sectionHasActiveItem(section) && 'text-foreground'
		);
	}

	function collapsedWorkspaceTriggerClass(workspace: SidebarWorkspaceSection): string {
		return cn(
			buttonVariants({ variant: 'ghost', size: 'icon' }),
			'relative flex h-10 w-10 rounded-lg',
			workspaceHasActiveItem(workspace)
				? 'bg-primary text-primary-foreground hover:bg-primary/90 hover:text-primary-foreground'
				: 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
		);
	}

	function navigateToMenuItem(item: SidebarMenuItem) {
		handleNavClick();
		void goto(resolve(item.path as any));
	}

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
	class="fixed left-0 top-0 z-50 h-screen bg-card border-r border-border transition-[width,transform,translate] duration-300 ease-in-out
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

		<Button
			variant="ghost"
			size="icon-sm"
			onclick={toggleSidebar}
			class="hidden rounded-lg lg:flex {isCollapsed ? 'mx-auto' : ''}"
			aria-label={isCollapsed ? 'Expand sidebar' : 'Collapse sidebar'}
		>
			<div class="transition-transform duration-300 {isCollapsed ? 'rotate-180' : ''}">
				<ChevronLeft class="w-5 h-5" />
			</div>
		</Button>
	</div>

	<!-- Navigation -->
	<Tooltip.Provider>
		<nav
			class={cn(
				'flex-1 overflow-y-auto overflow-x-hidden py-4 sidebar-nav',
				isCollapsed ? 'flex flex-col items-center gap-1 px-4' : 'space-y-1 px-4'
			)}
		>
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
				<Tooltip.Root delayDuration={0} disabled={!isCollapsed}>
					<Tooltip.Trigger class="w-full">
						<Button
							href="/staff/work"
							variant="ghost"
							onclick={handleNavClick}
							class={cn(
								'relative mb-3 h-11 rounded-lg px-0 transition-all duration-300',
								isCollapsed ? 'mx-auto w-10' : 'w-full justify-start',
								isActive('/staff/work')
									? 'bg-primary text-primary-foreground hover:bg-primary/90 hover:text-primary-foreground'
									: 'bg-accent/60 text-foreground hover:bg-accent'
							)}
						>
							<div class="w-[40px] h-[40px] flex items-center justify-center flex-shrink-0">
								<Inbox
									class="w-5 h-5 transition-colors {isActive('/staff/work')
										? 'text-primary-foreground'
										: 'text-muted-foreground group-hover:text-accent-foreground'}"
								/>
							</div>
							<span
								class="font-semibold whitespace-nowrap overflow-hidden transition-all duration-300 {isCollapsed
									? 'w-0 opacity-0 hidden'
									: 'w-auto opacity-100 ml-1'}"
							>
								งานของฉัน
							</span>
							{#if activeWorkCount > 0}
								<span
									class="absolute flex h-5 min-w-5 items-center justify-center rounded-full px-1.5 text-[11px] font-semibold leading-none {isCollapsed
										? '-right-1 -top-1'
										: 'right-2'} {urgentWorkCount > 0
										? 'bg-destructive text-destructive-foreground'
										: 'bg-primary text-primary-foreground'}"
								>
									{activeWorkCount > 99 ? '99+' : activeWorkCount}
								</span>
							{/if}
						</Button>
					</Tooltip.Trigger>
					{#if isCollapsed}
						<Tooltip.Content side="right" class="font-medium">งานของฉัน</Tooltip.Content>
					{/if}
				</Tooltip.Root>

				<!-- Workspace Menu Sections -->
				{#each workspaceSections as workspace (workspace.code)}
					{@const WorkspaceIcon = getIconComponent(workspace.icon)}
					{#if isCollapsed}
						<DropdownMenu.Root>
							<DropdownMenu.Trigger
								class={collapsedWorkspaceTriggerClass(workspace)}
								aria-label={workspace.name}
							>
								<WorkspaceIcon class="h-5 w-5" />
								{#if workspaceHasActiveItem(workspace)}
									<span class="absolute right-1 top-1 size-1.5 rounded-full bg-current"></span>
								{/if}
							</DropdownMenu.Trigger>
							<DropdownMenu.Content side="right" align="start" class="w-72">
								<DropdownMenu.Label class="px-2 py-1.5 text-sm font-semibold">
									{workspace.name}
								</DropdownMenu.Label>
								<DropdownMenu.Separator />
								{#each workspace.sections as section, sectionIndex (section.id)}
									{@const SectionIcon = getIconComponent(section.icon)}
									<DropdownMenu.Group>
										<DropdownMenu.Label
											class="flex items-center gap-2 px-2 py-1.5 text-xs font-semibold text-muted-foreground"
										>
											<SectionIcon class="h-3.5 w-3.5" />
											<span class="truncate">{section.name}</span>
										</DropdownMenu.Label>
										{#each section.items as item (item.id)}
											{@const Icon = getIconComponent(item.icon)}
											<DropdownMenu.Item
												onclick={() => navigateToMenuItem(item)}
												class={cn(
													'cursor-pointer gap-2',
													isActive(item.path) && 'bg-accent text-accent-foreground font-medium'
												)}
											>
												<Icon class="h-4 w-4" />
												<span class="truncate">{item.name}</span>
											</DropdownMenu.Item>
										{/each}
									</DropdownMenu.Group>
									{#if sectionIndex < workspace.sections.length - 1}
										<DropdownMenu.Separator />
									{/if}
								{/each}
							</DropdownMenu.Content>
						</DropdownMenu.Root>
					{:else}
						<div class="relative my-3 h-5 flex items-center px-3">
							<p
								class="absolute text-xs font-semibold text-muted-foreground uppercase whitespace-nowrap transition-opacity duration-300"
							>
								{workspace.name}
							</p>
						</div>

						{#each workspace.sections as section (section.id)}
							{@const SectionIcon = getIconComponent(section.icon)}
							<Button
								variant="ghost"
								onclick={() => toggleSection(section)}
								class={sectionTriggerClass(section)}
								aria-expanded={sectionExpanded(section)}
							>
								<SectionIcon class="h-4 w-4" />
								<span class="min-w-0 flex-1 truncate text-left">{section.name}</span>
								<ChevronDown
									class={cn(
										'ml-auto h-4 w-4 transition-transform',
										sectionExpanded(section) && 'rotate-180'
									)}
								/>
							</Button>
							{#if sectionExpanded(section)}
								<div class="ml-4 space-y-1 border-l border-border/70 pl-2">
									{#each section.items as item (item.id)}
										{@const Icon = getIconComponent(item.icon)}
										<Button
											href={item.path}
											variant="ghost"
											onclick={handleNavClick}
											class={navItemClass(item, true)}
										>
											<Icon class="h-4 w-4" />
											<span class="truncate">{item.name}</span>
										</Button>
									{/each}
								</div>
							{/if}
						{/each}
					{/if}
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
