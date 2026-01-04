<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Tabs } from '$lib/components/ui/tabs';
	import { ArrowLeft, Lock, Save, Eye, EyeOff } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';

	let currentPassword = $state('');
	let newPassword = $state('');
	let confirmPassword = $state('');
	let showCurrentPassword = $state(false);
	let showNewPassword = $state(false);
	let showConfirmPassword = $state(false);
	let saving = $state(false);

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
			// TODO: Implement API call to change password
			await new Promise((resolve) => setTimeout(resolve, 1000)); // Simulated API call
			toast.success('เปลี่ยนรหัสผ่านสำเร็จ');

			// Clear form
			currentPassword = '';
			newPassword = '';
			confirmPassword = '';
		} catch (error) {
			toast.error('ไม่สามารถเปลี่ยนรหัสผ่านได้');
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
			<Button variant="ghost" size="icon" onclick={() => goto(resolve('/dashboard'))}>
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
				<Button variant="secondary" class="w-full justify-start">
					<Lock class="w-4 h-4 mr-2" />
					ความปลอดภัย
				</Button>
				<!-- Future categories can be added here -->
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
		</div>
	</div>
</div>
