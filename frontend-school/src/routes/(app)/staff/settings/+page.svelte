<script lang="ts">
	import { onMount } from 'svelte';
	import { authAPI } from '$lib/api/auth';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import { Tabs } from '$lib/components/ui/tabs';
	import {
		ArrowLeft,
		Lock,
		Save,
		Eye,
		EyeOff,
		Download,
		Smartphone,
		CheckCircle2
	} from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { pwaStore } from '$lib/stores/pwa';

	let currentPassword = $state('');
	let newPassword = $state('');
	let confirmPassword = $state('');
	let showCurrentPassword = $state(false);
	let showNewPassword = $state(false);
	let showConfirmPassword = $state(false);
	let saving = $state(false);

	// Active tab
	let activeTab = $state<'security' | 'app'>('security');

	// PWA state from store - use $derived to avoid infinite loop
	let pwaState = $derived($pwaStore);
	let isInstalling = $state(false);

	// iOS detection
	let isIOS = $state(false);
	let isStandalone = $state(false);

	onMount(() => {
		// Check if iOS
		isIOS = /iPad|iPhone|iPod/.test(navigator.userAgent) && !(window as any).MSStream;

		// Check if already installed
		isStandalone =
			window.matchMedia('(display-mode: standalone)').matches ||
			(navigator as any).standalone === true;
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
		} catch (error) {
			toast.error('ไม่สามารถติดตั้งแอปได้');
		} finally {
			pwaStore.setPrompt(null);
			isInstalling = false;
		}
	}

	async function handleChangePassword(e: Event) {
		e.preventDefault();

		// Validation
		if (!currentPassword || !newPassword || !confirmPassword) {
			toast.error('กรุณากรอกข้อมูลให้ครบถ้วน');
			return;
		}

		if (newPassword !== confirmPassword) {
			toast.error('รหัสผ่านใหม่ไม่ตรงกัน');
			return;
		}

		if (newPassword.length < 8) {
			toast.error('รหัสผ่านต้องมีอย่างน้อย 8 ตัวอักษร');
			return;
		}

		saving = true;

		try {
			await authAPI.changePassword({
				currentPassword,
				newPassword
			});

			toast.success('เปลี่ยนรหัสผ่านสำเร็จ');

			// Clear form
			currentPassword = '';
			newPassword = '';
			confirmPassword = '';
		} catch (error) {
			const errorMessage = error instanceof Error ? error.message : 'ไม่สามารถเปลี่ยนรหัสผ่านได้';
			toast.error(errorMessage);
			console.error('Failed to change password:', error);
		} finally {
			saving = false;
		}
	}
</script>

<svelte:head>
	<title>การตั้งค่า - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-4">
			<Button variant="ghost" size="icon" onclick={() => goto(resolve('/staff'))}>
				<ArrowLeft class="h-5 w-5" />
			</Button>
			<div>
				<h1 class="text-3xl font-bold text-foreground">การตั้งค่า</h1>
				<p class="text-muted-foreground mt-1">จัดการการตั้งค่าบัญชีและความปลอดภัย</p>
			</div>
		</div>
	</div>

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
				<!-- Future categories -->
				<Button variant="ghost" class="w-full justify-start" disabled>
					<span class="text-muted-foreground">การแจ้งเตือน (เร็วๆ นี้)</span>
				</Button>
				<Button variant="ghost" class="w-full justify-start" disabled>
					<span class="text-muted-foreground">ความเป็นส่วนตัว (เร็วๆ นี้)</span>
				</Button>
			</CardContent>
		</Card>

		<!-- Main Content -->
		<div class="lg:col-span-3 space-y-6">
			{#if activeTab === 'security'}
				<!-- Change Password Section -->
				<Card>
					<CardHeader>
						<CardTitle>เปลี่ยนรหัสผ่าน</CardTitle>
						<CardDescription>อัพเดทรหัสผ่านของคุณเพื่อความปลอดภัยที่ดีขึ้น</CardDescription>
					</CardHeader>
					<CardContent>
						<form onsubmit={handleChangePassword} class="space-y-4">
							<!-- Current Password -->
							<div class="space-y-2">
								<Label for="currentPassword">รหัสผ่านปัจจุบัน *</Label>
								<div class="relative">
									<Input
										id="currentPassword"
										type={showCurrentPassword ? 'text' : 'password'}
										bind:value={currentPassword}
										placeholder="รหัสผ่านปัจจุบัน"
										required
										class="pr-10"
									/>
									<button
										type="button"
										onclick={() => (showCurrentPassword = !showCurrentPassword)}
										class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
									>
										{#if showCurrentPassword}
											<EyeOff class="w-4 h-4" />
										{:else}
											<Eye class="w-4 h-4" />
										{/if}
									</button>
								</div>
							</div>

							<!-- New Password -->
							<div class="space-y-2">
								<Label for="newPassword">รหัสผ่านใหม่ *</Label>
								<div class="relative">
									<Input
										id="newPassword"
										type={showNewPassword ? 'text' : 'password'}
										bind:value={newPassword}
										placeholder="รหัสผ่านใหม่ (อย่างน้อย 8 ตัวอักษร)"
										required
										minlength={8}
										class="pr-10"
									/>
									<button
										type="button"
										onclick={() => (showNewPassword = !showNewPassword)}
										class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
									>
										{#if showNewPassword}
											<EyeOff class="w-4 h-4" />
										{:else}
											<Eye class="w-4 h-4" />
										{/if}
									</button>
								</div>
							</div>

							<!-- Confirm Password -->
							<div class="space-y-2">
								<Label for="confirmPassword">ยืนยันรหัสผ่านใหม่ *</Label>
								<div class="relative">
									<Input
										id="confirmPassword"
										type={showConfirmPassword ? 'text' : 'password'}
										bind:value={confirmPassword}
										placeholder="ยืนยันรหัสผ่านใหม่"
										required
										class="pr-10"
									/>
									<button
										type="button"
										onclick={() => (showConfirmPassword = !showConfirmPassword)}
										class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
									>
										{#if showConfirmPassword}
											<EyeOff class="w-4 h-4" />
										{:else}
											<Eye class="w-4 h-4" />
										{/if}
									</button>
								</div>
							</div>

							<!-- Password Requirements -->
							<div class="bg-muted p-4 rounded-lg">
								<p class="text-sm font-medium mb-2">ข้อกำหนดรหัสผ่าน:</p>
								<ul class="text-sm text-muted-foreground space-y-1 list-disc list-inside">
									<li>มีความยาวอย่างน้อย 8 ตัวอักษร</li>
									<li>ควรประกอบด้วยตัวอักษรพิมพ์ใหญ่และพิมพ์เล็ก</li>
									<li>ควรมีตัวเลขและอักขระพิเศษ</li>
								</ul>
							</div>

							<!-- Submit Button -->
							<div class="flex justify-end">
								<Button type="submit" disabled={saving} class="gap-2">
									<Save class="h-4 w-4" />
									{saving ? 'กำลังบันทึก...' : 'เปลี่ยนรหัสผ่าน'}
								</Button>
							</div>
						</form>
					</CardContent>
				</Card>

				<!-- Security Tips -->
				<Card>
					<CardHeader>
						<CardTitle>เคล็ดลับความปลอดภัย</CardTitle>
					</CardHeader>
					<CardContent class="space-y-3">
						<div class="flex gap-3">
							<div class="flex-shrink-0">
								<div class="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center">
									<Lock class="w-4 h-4 text-primary" />
								</div>
							</div>
							<div>
								<p class="font-medium text-sm">เปลี่ยนรหัสผ่านเป็นประจำ</p>
								<p class="text-sm text-muted-foreground">แนะนำให้เปลี่ยนรหัสผ่านทุก 3-6 เดือน</p>
							</div>
						</div>
						<div class="flex gap-3">
							<div class="flex-shrink-0">
								<div class="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center">
									<Lock class="w-4 h-4 text-primary" />
								</div>
							</div>
							<div>
								<p class="font-medium text-sm">อย่าแชร์รหัสผ่าน</p>
								<p class="text-sm text-muted-foreground">
									อย่าให้รหัสผ่านของคุณกับใครก็ตาม รวมถึงผู้ดูแลระบบ
								</p>
							</div>
						</div>
						<div class="flex gap-3">
							<div class="flex-shrink-0">
								<div class="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center">
									<Lock class="w-4 h-4 text-primary" />
								</div>
							</div>
							<div>
								<p class="font-medium text-sm">ใช้รหัสผ่านที่แข็งแรง</p>
								<p class="text-sm text-muted-foreground">ผสมผสานตัวอักษร ตัวเลข และอักขระพิเศษ</p>
							</div>
						</div>
					</CardContent>
				</Card>
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
								<Button onclick={handleInstallPWA} disabled={isInstalling} class="w-full gap-2">
									<Download class="w-4 h-4" />
									{isInstalling ? 'กำลังติดตั้ง...' : 'ติดตั้งแอป'}
								</Button>
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
</div>
