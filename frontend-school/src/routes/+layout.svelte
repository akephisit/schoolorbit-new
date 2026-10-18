<script lang="ts">
	import './layout.css';
	import { Toaster } from 'svelte-sonner';
	import { page } from '$app/state';
	import { browser } from '$app/env';
	import { uiPreferences } from '#lib/stores/ui-preferences.js';
	import { onMount } from 'svelte';
	import InstallPrompt from '#lib/components/pwa/InstallPrompt.svelte';
	import IOSInstallPrompt from '#lib/components/pwa/IOSInstallPrompt.svelte';
	import MaintenancePage from '#lib/components/app-state/MaintenancePage.svelte';
	import {
		deploymentState,
		startDeploymentMonitor,
		stopDeploymentMonitor
	} from '#lib/deployment/maintenance.js';
	import { getServiceWorkerRegistration } from '#lib/pwa/service-worker-registration.js';
	import { initPWA } from '#lib/stores/pwa.js';
	import { startSchoolAppearance } from '#lib/appearance/runtime.js';
	import { applyPersonalMode } from '#lib/appearance/mode.js';

	let { children } = $props();
	let systemDark = $state(false);

	// Initialize PWA listeners once
	onMount(() => {
		const stopAppearance = startSchoolAppearance();
		const media = window.matchMedia('(prefers-color-scheme: dark)');
		systemDark = media.matches;
		const updateSystemTheme = () => {
			systemDark = media.matches;
		};
		media.addEventListener('change', updateSystemTheme);
		initPWA();
		if (import.meta.env.PROD && 'serviceWorker' in navigator) {
			void getServiceWorkerRegistration().catch((error: unknown) => {
				console.warn('ระบบแจ้งเตือนยังไม่พร้อม สามารถใช้เว็บและลองเปิดแจ้งเตือนอีกครั้งได้', error);
			});
		}
		document.documentElement.dataset.schoolorbitAppMounted = 'true';
		void startDeploymentMonitor();
		return () => {
			delete document.documentElement.dataset.schoolorbitAppMounted;
			stopDeploymentMonitor();
			stopAppearance();
			media.removeEventListener('change', updateSystemTheme);
		};
	});

	// One adapter applies the existing personal mode before SPA route DOM updates.
	$effect.pre(() => {
		const preference = $uiPreferences.theme;
		const path = page.url.pathname;
		if (!browser) return;
		if (preference === 'system') void systemDark;
		applyPersonalMode(preference, path);
	});
</script>

<svelte:head>
	{#if page.url.pathname !== '/'}
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
		<meta property="og:image" content={`${page.url.origin}/og-image.png`} />
		<meta property="og:image:width" content="1200" />
		<meta property="og:image:height" content="630" />
		<meta property="og:image:type" content="image/png" />
		<meta property="og:image:alt" content="ข้อมูลโรงเรียนและระบบจัดการโรงเรียนครบวงจร" />

		<!-- Twitter / X Card -->
		<meta name="twitter:card" content="summary_large_image" />
		<meta
			name="twitter:title"
			content={page.data.title ? `${page.data.title} - SchoolOrbit` : 'SchoolOrbit'}
		/>
		<meta
			name="twitter:description"
			content={page.data.description ?? 'ระบบจัดการโรงเรียนแบบครบวงจร'}
		/>
		<meta name="twitter:image" content={`${page.url.origin}/og-image.png`} />
		<meta name="twitter:image:alt" content="ข้อมูลโรงเรียนและระบบจัดการโรงเรียนครบวงจร" />
	{/if}
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
