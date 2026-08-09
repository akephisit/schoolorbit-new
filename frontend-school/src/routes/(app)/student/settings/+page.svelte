<script lang="ts">
	import { onMount } from 'svelte';
	import { Button } from '$lib/components/ui/button';
	import { PageShell } from '$lib/components/app-layout';
	import { LoadingButton } from '$lib/components/app-state';
	import PushNotificationSettings from '$lib/components/settings/PushNotificationSettings.svelte';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import {
		ArrowRight,
		Lock,
		Download,
		Smartphone,
		CheckCircle2,
		BellRing,
		ShieldCheck
	} from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { pwaStore } from '$lib/stores/pwa';

	// Active tab
	let activeTab = $state<'security' | 'app' | 'notifications'>('security');

	// PWA state from store - use $derived to avoid infinite loop
	let pwaState = $derived($pwaStore);
	let isInstalling = $state(false);

	// iOS detection
	let isIOS = $state(false);
	let isStandalone = $state(false);

	onMount(() => {
		// Check if iOS
		isIOS = /iPad|iPhone|iPod/.test(navigator.userAgent) && !window.MSStream;

		// Check if already installed
		isStandalone =
			window.matchMedia('(display-mode: standalone)').matches || navigator.standalone === true;
	});

	async function handleInstallPWA() {
		if (!pwaState.deferredPrompt) return;

		isInstalling = true;

		try {
			await pwaState.deferredPrompt.prompt();
			const choiceResult = await pwaState.deferredPrompt.userChoice;

			if (choiceResult.outcome === 'accepted') {
				toast.success('ติดตั้งแอปสำเร็จ');
			}
		} catch {
			toast.error('ไม่สามารถติดตั้งแอปได้');
		} finally {
			pwaStore.setPrompt(null);
			isInstalling = false;
		}
	}
</script>

<PageShell title="การตั้งค่า" description="จัดการการตั้งค่าบัญชีและความปลอดภัย" backHref="/student">
	<!-- Settings Tabs -->
	<div class="grid gap-6 lg:grid-cols-4">
		<!-- Sidebar Navigation -->
		<Card class="lg:col-span-1 h-fit">
			<CardHeader>
				<CardTitle class="text-base">หมวดหมู่</CardTitle>
			</CardHeader>
			<CardContent class="space-y-1">
				<Button
					variant={activeTab === 'security' ? 'secondary' : 'ghost'}
					class="w-full justify-start"
					onclick={() => (activeTab = 'security')}
				>
					<Lock class="w-4 h-4 mr-2" />
					ความปลอดภัย
				</Button>
				<Button
					variant={activeTab === 'app' ? 'secondary' : 'ghost'}
					class="w-full justify-start"
					onclick={() => (activeTab = 'app')}
				>
					<Smartphone class="w-4 h-4 mr-2" />
					แอพพลิเคชัน
				</Button>
				<Button
					variant={activeTab === 'notifications' ? 'secondary' : 'ghost'}
					class="w-full justify-start"
					onclick={() => (activeTab = 'notifications')}
				>
					<BellRing class="w-4 h-4 mr-2" />
					การแจ้งเตือน
				</Button>
				<!-- Future categories -->
				<Button variant="ghost" class="w-full justify-start" disabled>
					<span class="text-muted-foreground">ความเป็นส่วนตัว (เร็วๆ นี้)</span>
				</Button>
			</CardContent>
		</Card>

		<!-- Main Content -->
		<div class="lg:col-span-3 space-y-6">
			{#if activeTab === 'security'}
				<Card>
					<CardHeader>
						<div class="bg-primary/10 mb-2 flex h-10 w-10 items-center justify-center rounded-lg">
							<ShieldCheck class="text-primary h-5 w-5" />
						</div>
						<CardTitle>ความปลอดภัยของบัญชี</CardTitle>
						<CardDescription>
							เปลี่ยนรหัสผ่าน ตรวจสอบอุปกรณ์ และออกจากระบบอุปกรณ์ที่ไม่รู้จักได้จากศูนย์กลางเดียว
						</CardDescription>
					</CardHeader>
					<CardContent>
						<Button href="/account/security" class="gap-2">
							จัดการความปลอดภัยของบัญชี
							<ArrowRight class="h-4 w-4" />
						</Button>
					</CardContent>
				</Card>
			{:else if activeTab === 'notifications'}
				<PushNotificationSettings />
			{:else if activeTab === 'app'}
				<!-- PWA Installation -->
				<Card>
					<CardHeader>
						<CardTitle>ติดตั้งแอป</CardTitle>
						<CardDescription>
							ติดตั้ง SchoolOrbit เป็นแอปบนอุปกรณ์ของคุณเพื่อการเข้าถึงที่รวดเร็วยิ่งขึ้น
						</CardDescription>
					</CardHeader>
					<CardContent class="space-y-4">
						{#if pwaState.isInstalled}
							<!-- Already Installed -->
							<div
								class="flex items-center gap-3 p-4 bg-green-500/10 border border-green-500/20 rounded-lg"
							>
								<div class="bg-green-500/20 p-2 rounded-lg">
									<CheckCircle2 class="w-5 h-5 text-green-600 dark:text-green-400" />
								</div>
								<div class="flex-1">
									<p class="font-medium text-sm text-green-900 dark:text-green-100">
										แอปถูกติดตั้งแล้ว
									</p>
									<p class="text-xs text-green-700 dark:text-green-300 mt-0.5">
										คุณกำลังใช้งาน SchoolOrbit ในโหมดแอป
									</p>
								</div>
							</div>
						{:else if pwaState.deferredPrompt}
							<!-- Can Install -->
							<div class="space-y-3">
								<div class="flex items-start gap-3">
									<div class="bg-primary/10 p-2 rounded-lg flex-shrink-0 mt-0.5">
										<Smartphone class="w-5 h-5 text-primary" />
									</div>
									<div class="flex-1">
										<p class="text-sm text-muted-foreground">
											ติดตั้งแอป SchoolOrbit บนอุปกรณ์ของคุณเพื่อ:
										</p>
										<ul class="text-sm text-muted-foreground list-disc list-inside mt-2 space-y-1">
											<li>เข้าถึงได้เร็วขึ้นจากหน้าจอโฮม</li>
											<li>ทำงานแบบ full screen</li>
											<li>ประสบการณ์การใช้งานแบบ native app</li>
										</ul>
									</div>
								</div>
								<LoadingButton
									onclick={handleInstallPWA}
									loading={isInstalling}
									loadingLabel="กำลังติดตั้ง..."
									class="w-full gap-2"
								>
									<Download class="w-4 h-4" />
									ติดตั้งแอป
								</LoadingButton>
							</div>
						{:else}
							<!-- Not Available (Show iOS instructions if on iOS) -->
							{#if isIOS && !isStandalone}
								<!-- iOS Manual Install Instructions -->
								<div class="space-y-3">
									<div class="flex items-start gap-3">
										<div class="bg-blue-500/10 p-2 rounded-lg flex-shrink-0 mt-0.5">
											<svg
												class="w-5 h-5 text-blue-600"
												fill="none"
												viewBox="0 0 24 24"
												stroke="currentColor"
											>
												<path
													stroke-linecap="round"
													stroke-linejoin="round"
													stroke-width="2"
													d="M8.684 13.342C8.886 12.938 9 12.482 9 12c0-.482-.114-.938-.316-1.342m0 2.684a3 3 0 110-2.684m0 2.684l6.632 3.316m-6.632-6l6.632-3.316m0 0a3 3 0 105.367-2.684 3 3 0 00-5.367 2.684zm0 9.316a3 3 0 105.368 2.684 3 3 0 00-5.368-2.684z"
												/>
											</svg>
										</div>
										<div class="flex-1">
											<p class="text-sm font-medium text-foreground">วิธีติดตั้งบน iOS/Safari:</p>
											<ol
												class="text-sm text-muted-foreground mt-2 space-y-1.5 list-decimal list-inside"
											>
												<li>กดปุ่ม <strong>แชร์</strong> (Share) ที่แถบเมนู Safari</li>
												<li>เลื่อนลงและเลือก <strong>"เพิ่มที่หน้าจอโฮม"</strong></li>
												<li>กดปุ่ม <strong>"เพิ่ม"</strong> ที่มุมขวาบน</li>
											</ol>
										</div>
									</div>
								</div>
							{:else}
								<div class="p-4 bg-muted rounded-lg">
									<p class="text-sm text-muted-foreground text-center">
										ตัวเลือกการติดตั้งจะปรากฏเมื่อเปิดเว็บไซต์ในเบราว์เซอร์ที่รองรับ
									</p>
								</div>
							{/if}
						{/if}
					</CardContent>
				</Card>
			{/if}
		</div>
	</div>
</PageShell>
