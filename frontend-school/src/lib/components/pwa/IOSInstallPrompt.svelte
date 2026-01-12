<script lang="ts">
	import { onMount } from 'svelte';
	import { Button } from '$lib/components/ui/button';
	import { X, Share, Plus } from 'lucide-svelte';
	import { fade, slide } from 'svelte/transition';

	let showIOSPrompt = $state(false);
	let isIOS = $state(false);
	let isStandalone = $state(false);

	onMount(() => {
		// Check if iOS
		isIOS = /iPad|iPhone|iPod/.test(navigator.userAgent) && !(window as any).MSStream;

		// Check if already installed (standalone mode)
		isStandalone =
			window.matchMedia('(display-mode: standalone)').matches ||
			(navigator as any).standalone === true;

		// Show prompt if iOS and not installed
		if (isIOS && !isStandalone) {
			// Check if user dismissed before
			const dismissed = localStorage.getItem('ios-install-dismissed');
			const dismissedTime = dismissed ? parseInt(dismissed) : 0;
			const now = Date.now();

			// Show if not dismissed or dismissed more than 7 days ago
			if (!dismissed || now - dismissedTime > 7 * 24 * 60 * 60 * 1000) {
				// Show after 3 seconds delay
				setTimeout(() => {
					showIOSPrompt = true;
				}, 3000);
			}
		}
	});

	function handleDismiss() {
		showIOSPrompt = false;
		localStorage.setItem('ios-install-dismissed', Date.now().toString());
	}
</script>

{#if showIOSPrompt}
	<div
		class="fixed bottom-4 left-4 right-4 z-50 mx-auto max-w-sm"
		transition:slide={{ duration: 300 }}
	>
		<div
			class="bg-card border border-border rounded-lg shadow-xl p-4 space-y-3"
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
						<Share class="w-5 h-5 text-primary" />
					</div>
					<div class="flex-1">
						<h3 class="font-semibold text-sm">ติดตั้ง SchoolOrbit</h3>
						<p class="text-xs text-muted-foreground mt-1">
							ติดตั้งแอปบนหน้าจอโฮมเพื่อการเข้าถึงที่รวดเร็วยิ่งขึ้น
						</p>
					</div>
				</div>
			</div>

			<!-- Instructions -->
			<div class="bg-muted/50 rounded-lg p-3 space-y-2">
				<p class="text-xs font-medium text-foreground">วิธีติดตั้ง:</p>
				<ol class="text-xs text-muted-foreground space-y-1.5 list-none">
					<li class="flex items-start gap-2">
						<span class="font-semibold text-primary">1.</span>
						<span class="flex-1">
							กดปุ่ม <Share class="w-3 h-3 inline mx-0.5" /> (แชร์) ที่แถบเมนู Safari
						</span>
					</li>
					<li class="flex items-start gap-2">
						<span class="font-semibold text-primary">2.</span>
						<span class="flex-1">
							เลื่อนลงและเลือก "เพิ่มที่หน้าจอโฮม" <Plus class="w-3 h-3 inline mx-0.5" />
						</span>
					</li>
					<li class="flex items-start gap-2">
						<span class="font-semibold text-primary">3.</span>
						<span class="flex-1">กดปุ่ม "เพิ่ม" ที่มุมขวาบน</span>
					</li>
				</ol>
			</div>

			<!-- Actions -->
			<div class="flex gap-2 pt-1">
				<Button variant="ghost" size="sm" onclick={handleDismiss} class="flex-1 text-xs">
					ไว้ทีหลัง
				</Button>
				<Button variant="default" size="sm" onclick={handleDismiss} class="flex-1 text-xs" disabled>
					เข้าใจแล้ว
				</Button>
			</div>
		</div>
	</div>
{/if}
