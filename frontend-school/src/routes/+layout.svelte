<script lang="ts">
	import './layout.css';
	import favicon from '$lib/assets/favicon.svg';
	import { Toaster } from 'svelte-sonner';
	import { page } from '$app/state';
	import { onMount } from 'svelte';

	let { children } = $props();

	// Force light theme for public pages (landing, login)
	onMount(() => {
		const isPublicPage = page.url.pathname === '/' || page.url.pathname.startsWith('/login');
		
		if (isPublicPage) {
			// Remove dark class for public pages
			document.documentElement.classList.remove('dark');
		}
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<!-- Sonner Toaster for global notifications -->
<Toaster position="bottom-right" richColors />

{@render children()}
