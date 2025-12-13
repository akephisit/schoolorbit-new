<script lang="ts">
	import { authStore } from '$lib/stores/auth.svelte';
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';

	let { children } = $props();
	
	// Current route tracking
	let currentPath = $derived($page.url.pathname);
	
	// Protect routes - redirect to login if not authenticated
	onMount(() => {
		if (!authStore.isAuthenticated) {
			goto('/login');
		}
	});

	function handleLogout() {
		authStore.logout();
	}
	
	// Navigation items
	const navItems = [
		{ href: '/dashboard', label: 'หน้าหลัก', icon: 'M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6' },
		{ href: '/dashboard/schools', label: 'จัดการโรงเรียน', icon: 'M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4' }
	];
	
	function isActive(href: string): boolean {
		if (href === '/dashboard') {
			return currentPath === '/dashboard';
		}
		return currentPath.startsWith(href);
	}
</script>

{#if authStore.isAuthenticated && authStore.user}
	<div class="min-h-screen bg-gray-50">
		<!-- Top Navigation Bar -->
		<nav class="bg-gradient-to-r from-indigo-600 to-purple-600 shadow-lg">
			<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
				<div class="flex justify-between h-16">
					<!-- Left: Logo & Navigation -->
					<div class="flex items-center space-x-8">
						<!-- Logo/Brand -->
						<div class="flex-shrink-0 flex items-center">
							<svg class="h-8 w-8 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									stroke-width="2"
									d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"
								/>
							</svg>
							<span class="ml-3 text-xl font-bold text-white">SchoolOrbit</span>
							<span class="ml-2 px-2 py-0.5 bg-white bg-opacity-20 rounded text-xs text-white"
								>Admin</span
							>
						</div>

						<!-- Navigation Links -->
						<div class="hidden md:flex md:space-x-2">
							{#each navItems as item}
								<a href={item.href} class="nav-link {isActive(item.href) ? 'active' : ''}">
									<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
										<path
											stroke-linecap="round"
											stroke-linejoin="round"
											stroke-width="2"
											d={item.icon}
										/>
									</svg>
									<span>{item.label}</span>
								</a>
							{/each}
						</div>
					</div>

					<!-- Right: User Info & Logout -->
					<div class="flex items-center space-x-4">
						<!-- User Info -->
						<div
							class="hidden md:flex items-center space-x-3 px-4 py-2 bg-white bg-opacity-90 rounded-lg shadow-md"
						>
							<div
								class="h-8 w-8 rounded-full bg-gradient-to-r from-indigo-500 to-purple-500 flex items-center justify-center"
							>
								<svg class="h-5 w-5 text-white" fill="currentColor" viewBox="0 0 20 20">
									<path
										fill-rule="evenodd"
										d="M10 9a3 3 0 100-6 3 3 0 000 6zm-7 9a7 7 0 1114 0H3z"
										clip-rule="evenodd"
									/>
								</svg>
							</div>
							<div class="text-left">
								<div class="text-sm font-medium text-gray-800">{authStore.user.name}</div>
								<div class="text-xs text-gray-600">{authStore.user.role}</div>
							</div>
						</div>

						<!-- Logout Button -->
						<button
							onclick={handleLogout}
							class="px-4 py-2 bg-white bg-opacity-20 hover:bg-opacity-30 text-white rounded-lg transition-all flex items-center space-x-2"
						>
							<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									stroke-width="2"
									d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"
								/>
							</svg>
							<span>ออกจากระบบ</span>
						</button>
					</div>
				</div>
			</div>
		</nav>

		<!-- Main Content -->
		<main class="max-w-7xl mx-auto px-4 py-6 sm:px-6 lg:px-8">
			{@render children()}
		</main>
	</div>
{:else}
	<div class="min-h-screen flex items-center justify-center">
		<div class="text-center">
			<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-indigo-600 mx-auto"></div>
			<p class="mt-4 text-gray-600">กำลังโหลด...</p>
		</div>
	</div>
{/if}

<style>
	.nav-link {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 1rem;
		border-radius: 0.5rem;
		color: white;
		font-weight: 500;
		transition: all 0.2s;
		text-decoration: none;
	}

	.nav-link:hover {
		background-color: rgba(255, 255, 255, 0.1);
	}

	.nav-link.active {
		background-color: rgba(255, 255, 255, 0.2);
		box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
	}
</style>
