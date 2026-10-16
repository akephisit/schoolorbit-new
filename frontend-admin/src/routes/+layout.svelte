<script lang="ts">
	import './layout.css';
	import { Toaster } from 'svelte-sonner';

	let { children } = $props();
</script>

<svelte:head>
	<link rel="preconnect" href="https://fonts.googleapis.com" />
	<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
	<link
		href="https://fonts.googleapis.com/css2?family=Kanit:wght@300;400;500;600&display=swap"
		rel="stylesheet"
	/>
</svelte:head>

<Toaster richColors position="top-right" />

{@render children()}
