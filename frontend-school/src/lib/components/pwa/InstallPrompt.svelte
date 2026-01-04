<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { X, Download } from 'lucide-svelte';
	import { fade, slide } from 'svelte/transition';
	import { pwaStore, type BeforeInstallPromptEvent } from '$lib/stores/pwa';

	let showInstallPrompt = $state(false);
	let isInstalling = $state(false);

	// Use $derived instead of $effect to avoid infinite loop
	let pwaState = $derived($pwaStore);

	// Watch for changes and update visibility
	$effect(() => {
		// Show prompt when deferredPrompt is available
		if (pwaState.deferredPrompt && !pwaState.isInstalled) {
			// Check if user has dismissed this before
			const dismissed = localStorage.getItem('pwa-install-dismissed');
			const dismissedTime = dismissed ? parseInt(dismissed) : 0;
			const now = Date.now();
			
			// Show prompt if not dismissed in last 7 days
			if (!dismissed || now - dismissedTime > 7 * 24 * 60 * 60 * 1000) {
				showInstallPrompt = true;
			}
		} else {
			showInstallPrompt = false;
		}
	});

	async function handleInstall() {
		if (!pwaState.deferredPrompt) return;

		isInstalling = true;

		try {
			// Show the install prompt
			await pwaState.deferredPrompt.prompt();

			// Wait for user choice
			const choiceResult = await pwaState.deferredPrompt.userChoice;

			if (choiceResult.outcome === 'accepted') {
				console.log('✅ User accepted the install prompt');
			} else {
				console.log('❌ User dismissed the install prompt');
			}
		} catch (error) {
			console.error('Install error:', error);
		} finally {
			// Clear the prompt
			pwaStore.setPrompt(null);
			showInstallPrompt = false;
			isInstalling = false;
		}
	}

	function handleDismiss() {
		showInstallPrompt = false;
		pwaStore.setPrompt(null);
		// Remember dismissal for 7 days
		localStorage.setItem('pwa-install-dismissed', Date.now().toString());
	}
</script>

{#if showInstallPrompt}
	<div class="fixed bottom-4 right-4 z-50 max-w-sm" transition:slide={{ duration: 300 }}>
		<div
			class="bg-card border border-border rounded-lg shadow-lg p-4 space-y-3"
			transition:fade={{ duration: 200 }}
		>
			<!-- Close button -->
			<button
				onclick={handleDismiss}
				class="absolute top-2 right-2 text-muted-foreground hover:text-foreground transition-colors"
				aria-label="ปิด"
			>
				<X class="w-4 h-4" />
			</button>

			<!-- Content -->
			<div class="pr-6">
				<div class="flex items-start gap-3">
					<div class="bg-primary/10 p-2 rounded-lg flex-shrink-0">
						<Download class="w-5 h-5 text-primary" />
					</div>
					<div class="flex-1">
						<h3 class="font-semibold text-sm">ติดตั้งแอป SchoolOrbit</h3>
						<p class="text-xs text-muted-foreground mt-1">
							ติดตั้งแอปบนหน้าจอโฮมเพื่อการเข้าถึงที่รวดเร็วยิ่งขึ้น
						</p>
					</div>
				</div>
			</div>

			<!-- Actions -->
			<div class="flex gap-2 pt-1">
				<Button variant="ghost" size="sm" onclick={handleDismiss} class="flex-1 text-xs">
					ไว้ทีหลัง
				</Button>
				<Button
					size="sm"
					onclick={handleInstall}
					disabled={isInstalling}
					class="flex-1 text-xs gap-2"
				>
					{#if isInstalling}
						กำลังติดตั้ง...
					{:else}
						<Download class="w-3 h-3" />
						ติดตั้ง
					{/if}
				</Button>
			</div>
		</div>
	</div>
{/if}
