<script lang="ts">
	import { Bell, Search, Menu, Sun, Moon } from 'lucide-svelte';
	import { Button } from '$lib/components/ui/button';
	import ProfileMenu from './ProfileMenu.svelte';
	import NotificationBell from './NotificationBell.svelte';
	import { uiPreferences } from '$lib/stores/ui-preferences';
	import { onMount } from 'svelte';

	interface Props {
		onMenuClick?: () => void;
		sidebarCollapsed?: boolean;
	}

	let { onMenuClick, sidebarCollapsed = false }: Props = $props();

	let isDarkMode = $state(false);

	// Load theme from localStorage on mount
	onMount(() => {
		const theme = $uiPreferences.theme;
		if (theme === 'dark') {
			isDarkMode = true;
			document.documentElement.classList.add('dark');
		} else if (theme === 'light') {
			isDarkMode = false;
			document.documentElement.classList.remove('dark');
		} else {
			// system preference
			const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
			isDarkMode = prefersDark;
			if (prefersDark) {
				document.documentElement.classList.add('dark');
			}
		}
	});

	function toggleDarkMode() {
		isDarkMode = !isDarkMode;
		// Toggle dark class on document
		if (isDarkMode) {
			document.documentElement.classList.add('dark');
			uiPreferences.setTheme('dark');
		} else {
			document.documentElement.classList.remove('dark');
			uiPreferences.setTheme('light');
		}
	}
</script>

<header
	class="sticky top-0 h-16 bg-card border-b border-border z-30 transition-all duration-300
	{sidebarCollapsed ? 'lg:ml-0' : 'lg:ml-0'}"
>
	<div class="h-full px-4 lg:px-6 flex items-center justify-between gap-4">
		<!-- Left Section -->
		<div class="flex items-center gap-4">
			<!-- Mobile Menu Button -->
			<Button
				variant="ghost"
				size="icon"
				onclick={onMenuClick}
				class="lg:hidden"
				aria-label="Open Menu"
			>
				<Menu class="w-5 h-5" />
			</Button>

			<!-- Search -->
			<div class="hidden md:flex items-center gap-2 bg-accent rounded-lg px-3 py-2 min-w-[300px]">
				<Search class="w-4 h-4 text-muted-foreground" />
				<input
					type="text"
					placeholder="ค้นหานักเรียน, ครู, รายวิชา..."
					class="bg-transparent border-none outline-none text-sm w-full text-foreground placeholder:text-muted-foreground"
				/>
			</div>
		</div>

		<!-- Right Section -->
		<div class="flex items-center gap-2">
			<!-- Search Button - Mobile Only -->
			<Button variant="ghost" size="icon" class="md:hidden" aria-label="Search">
				<Search class="w-5 h-5" />
			</Button>

			<!-- Dark Mode Toggle -->
			<Button variant="ghost" size="icon" onclick={toggleDarkMode} aria-label="Toggle Dark Mode">
				{#if isDarkMode}
					<Sun class="w-5 h-5" />
				{:else}
					<Moon class="w-5 h-5" />
				{/if}
			</Button>

			<!-- Notifications -->
			<div class="relative">
				<NotificationBell />
			</div>

			<!-- User Menu -->
			<div class="flex items-center pl-3 border-l border-border">
				<ProfileMenu />
			</div>
		</div>
	</div>
</header>
