<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { authStore } from '$lib/stores/auth.svelte';

	// Redirect to dashboard if already authenticated
	onMount(() => {
		if (authStore.isAuthenticated) {
			goto('/dashboard');
		}
	});

	// Minimal animation state using Svelte 5 runes
	let mounted = $state(false);

	onMount(() => {
		setTimeout(() => (mounted = true), 50);
	});

	function handleLogin() {
		goto('/login');
	}
</script>

<svelte:head>
	<title>SchoolOrbit Admin</title>
	<meta name="description" content="Modern school management system" />
</svelte:head>

<div class="min-h-screen bg-white flex items-center justify-center px-6">
	<main
		class="max-w-4xl w-full text-center space-y-12 transition-all duration-700"
		class:opacity-0={!mounted}
		class:translate-y-4={!mounted}
	>
		<!-- Logo mark -->
		<div class="inline-flex items-center justify-center">
			<div class="w-16 h-16 rounded-2xl bg-black flex items-center justify-center">
				<svg class="w-8 h-8 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="1.5"
						d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"
					/>
				</svg>
			</div>
		</div>

		<!-- Heading -->
		<div class="space-y-6">
			<h1 class="text-6xl md:text-7xl font-light tracking-tight text-black">
				School<span class="font-medium">Orbit</span>
			</h1>
			<p class="text-xl md:text-2xl text-gray-500 font-light max-w-2xl mx-auto leading-relaxed">
				Admin Portal
			</p>
		</div>

		<!-- Description -->
		<p class="text-base text-gray-600 max-w-md mx-auto leading-relaxed">
			ระบบบริหารจัดการโรงเรียนที่ออกแบบมาเพื่อความเรียบง่าย<br />
			รวดเร็ว และปลอดภัย
		</p>

		<!-- CTA -->
		<div class="pt-4">
			<button
				onclick={handleLogin}
				class="group inline-flex items-center gap-3 px-8 py-4 bg-black text-white rounded-full text-sm font-medium hover:bg-gray-900 transition-all duration-200 hover:gap-4"
			>
				เข้าสู่ระบบ
				<svg
					class="w-4 h-4 transition-transform group-hover:translate-x-0.5"
					fill="none"
					stroke="currentColor"
					viewBox="0 0 24 24"
				>
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="2"
						d="M13 7l5 5m0 0l-5 5m5-5H6"
					/>
				</svg>
			</button>
		</div>

		<!-- Features - Minimal list -->
		<div class="pt-16 border-t border-gray-200">
			<div class="grid grid-cols-1 md:grid-cols-3 gap-12 max-w-3xl mx-auto">
				<div class="space-y-2">
					<div class="text-sm font-medium text-black">Secure</div>
					<div class="text-sm text-gray-500 leading-relaxed">
						PDPA compliant with end-to-end encryption
					</div>
				</div>

				<div class="space-y-2">
					<div class="text-sm font-medium text-black">Fast</div>
					<div class="text-sm text-gray-500 leading-relaxed">
						Built with Rust for maximum performance
					</div>
				</div>

				<div class="space-y-2">
					<div class="text-sm font-medium text-black">Reliable</div>
					<div class="text-sm text-gray-500 leading-relaxed">99.9% uptime with 24/7 monitoring</div>
				</div>
			</div>
		</div>

		<!-- Footer info -->
		<div class="pt-8 text-xs text-gray-400">© 2024 SchoolOrbit. All rights reserved.</div>
	</main>
</div>

<style>
	/* Smooth font rendering */
	:global(body) {
		-webkit-font-smoothing: antialiased;
		-moz-osx-font-smoothing: grayscale;
	}
</style>
