<script lang="ts">
	import './layout.css';
	import { Toaster } from 'svelte-sonner';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import InstallPrompt from '$lib/components/pwa/InstallPrompt.svelte';
	import IOSInstallPrompt from '$lib/components/pwa/IOSInstallPrompt.svelte';
	import MaintenancePage from '$lib/components/app-state/MaintenancePage.svelte';
	import {
		deploymentState,
		startDeploymentMonitor,
		stopDeploymentMonitor
	} from '$lib/deployment/maintenance';
	import { initPWA } from '$lib/stores/pwa';

	let { children } = $props();

	// Initialize PWA listeners once
	onMount(() => {
		initPWA();
		document.documentElement.dataset.schoolorbitAppMounted = 'true';
		void startDeploymentMonitor();
		return () => {
			delete document.documentElement.dataset.schoolorbitAppMounted;
			stopDeploymentMonitor();
		};
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

<svelte:head>
	<!-- Open Graph -->
	<meta property="og:type" content="website" />
	<meta property="og:site_name" content="SchoolOrbit" />
	<meta property="og:locale" content="th_TH" />
	<meta property="og:url" content={page.url.href} />
	<meta
		property="og:title"
		content={page.data.title ? `${page.data.title} - SchoolOrbit` : 'SchoolOrbit'}
	/>
	<meta
		property="og:description"
		content={page.data.description ?? 'ระบบจัดการโรงเรียนแบบครบวงจร'}
	/>
	<meta property="og:image" content={`${page.url.origin}/icon-192.png`} />

	<!-- Twitter / X Card -->
	<meta name="twitter:card" content="summary" />
	<meta
		name="twitter:title"
		content={page.data.title ? `${page.data.title} - SchoolOrbit` : 'SchoolOrbit'}
	/>
	<meta
		name="twitter:description"
		content={page.data.description ?? 'ระบบจัดการโรงเรียนแบบครบวงจร'}
	/>
	<meta name="twitter:image" content={`${page.url.origin}/icon-192.png`} />
</svelte:head>

{#if $deploymentState.status === 'maintenance'}
	<MaintenancePage releaseId={$deploymentState.releaseId} />
{:else}
	<!-- Sonner Toaster for global notifications -->
	<Toaster position="bottom-right" richColors />

	<!-- PWA Install Prompt (Android/Desktop) -->
	<InstallPrompt />

	<!-- iOS Install Prompt (Safari) -->
	<IOSInstallPrompt />

	{@render children()}
{/if}
