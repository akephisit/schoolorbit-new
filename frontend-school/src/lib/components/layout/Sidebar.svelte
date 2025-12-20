<script lang="ts">
	import {
		LayoutDashboard,
		Users,
		GraduationCap,
		BookOpen,
		School,
		Calendar,
		Settings,
		LogOut,
		ChevronLeft,
		Menu
	} from 'lucide-svelte';
	import { Button } from '$lib/components/ui/button';

	let isCollapsed = $state(false);
	let isMobileOpen = $state(false);

	const navigation = [
		{ name: 'แดชบอร์ด', icon: LayoutDashboard, href: '/dashboard', active: true },
		{ name: 'นักเรียน', icon: Users, href: '/students', active: false },
		{ name: 'บุคลากร', icon: GraduationCap, href: '/staff', active: false },
		{ name: 'รายวิชา', icon: BookOpen, href: '/subjects', active: false },
		{ name: 'ห้องเรียน', icon: School, href: '/classes', active: false },
		{ name: 'ปฏิทิน', icon: Calendar, href: '/calendar', active: false }
	];

	const bottomNavigation = [
		{ name: 'ตั้งค่า', icon: Settings, href: '/settings' },
		{ name: 'ออกจากระบบ', icon: LogOut, href: '/logout' }
	];

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
		<div class="h-16 border-b border-border flex items-center justify-between px-6">
			{#if !isCollapsed}
				<div class="flex items-center gap-3">
					<div class="w-10 h-10 bg-primary rounded-lg flex items-center justify-center">
						<GraduationCap class="w-6 h-6 text-primary-foreground" />
					</div>
					<div>
						<h2 class="font-bold text-foreground text-lg">SchoolOrbit</h2>
						<p class="text-xs text-muted-foreground">ระบบจัดการโรงเรียน</p>
					</div>
				</div>
			{:else}
				<div class="w-10 h-10 bg-primary rounded-lg flex items-center justify-center mx-auto">
					<GraduationCap class="w-6 h-6 text-primary-foreground" />
				</div>
			{/if}

			<!-- Toggle Button - Desktop Only -->
			<button
				onclick={toggleSidebar}
				class="hidden lg:flex w-6 h-6 items-center justify-center rounded hover:bg-accent transition-colors"
				aria-label="Toggle Sidebar"
			>
				<ChevronLeft
					class="w-4 h-4 text-muted-foreground transition-transform {isCollapsed
						? 'rotate-180'
						: ''}"
				/>
			</button>
		</div>

		<!-- Navigation -->
		<nav class="flex-1 overflow-y-auto p-4 space-y-1">
			{#each navigation as item}
				{@const Icon = item.icon}
				<a
					href={item.href}
					class="flex items-center gap-3 px-3 py-2.5 rounded-lg transition-colors group
						{item.active
						? 'bg-primary text-primary-foreground'
						: 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'}"
				>
					<Icon
						class="w-5 h-5 flex-shrink-0 {item.active
							? 'text-primary-foreground'
							: 'text-muted-foreground group-hover:text-accent-foreground'}"
					/>
					{#if !isCollapsed}
						<span class="font-medium">{item.name}</span>
					{/if}
				</a>
			{/each}
		</nav>

		<!-- Bottom Navigation -->
		<div class="border-t border-border p-4 space-y-1">
			{#each bottomNavigation as item}
				{@const Icon = item.icon}
				<a
					href={item.href}
					class="flex items-center gap-3 px-3 py-2.5 rounded-lg transition-colors
						text-muted-foreground hover:bg-accent hover:text-accent-foreground group"
				>
					<Icon
						class="w-5 h-5 flex-shrink-0 text-muted-foreground group-hover:text-accent-foreground"
					/>
					{#if !isCollapsed}
						<span class="font-medium">{item.name}</span>
					{/if}
				</a>
			{/each}
		</div>
	</div>
</aside>
