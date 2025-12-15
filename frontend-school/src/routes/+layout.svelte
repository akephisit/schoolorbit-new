<script lang="ts">
	import './layout.css';
	import favicon from '$lib/assets/favicon.svg';
	import { Toaster } from 'svelte-sonner';
	import { onMount } from 'svelte';
	import { authAPI } from '$lib/api/auth';

	let { children } = $props();

	// Check authentication on mount
	onMount(async () => {
		await authAPI.checkAuth();
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<!-- Sonner Toaster for global notifications -->
<Toaster position="top-right" richColors />

{@render children()}
