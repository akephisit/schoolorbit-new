<script lang="ts">
	import './layout.css';
	import { Toaster } from 'svelte-sonner';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import InstallPrompt from '$lib/components/pwa/InstallPrompt.svelte';
	import IOSInstallPrompt from '$lib/components/pwa/IOSInstallPrompt.svelte';
	import { initPWA } from '$lib/stores/pwa';

	let { children } = $props();

	// Initialize PWA listeners once
	onMount(() => {
		initPWA();
	});

	// Force light theme for public pages (landing, login)
	// Using $effect instead of onMount to handle SPA navigation
	$effect(() => {
		const isPublicPage = page.url.pathname === '/' || page.url.pathname.startsWith('/login');

		if (isPublicPage) {
			// Remove dark class for public pages
			document.documentElement.classList.remove('dark');
		}
	});
</script>

<!-- Sonner Toaster for global notifications -->
<Toaster position="bottom-right" richColors />

<!-- PWA Install Prompt (Android/Desktop) -->
<InstallPrompt />

<!-- iOS Install Prompt (Safari) -->
<IOSInstallPrompt />

{@render children()}
