<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { GraduationCap, ArrowLeft } from 'lucide-svelte';
	import { authAPI } from '$lib/api/auth';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';

	import { authStore } from '$lib/stores/auth';

	let nationalId = $state('');
	let password = $state('');
	let rememberMe = $state(false);
	let isLoading = $state(false);
	let errorMessage = $state('');
	let isCheckingAuth = $state(true);
	let redirectUrl = $state<string | null>(null);

	// Check if user is already authenticated
	onMount(async () => {
		// Get redirect URL from sessionStorage (set by route guards)
		redirectUrl = sessionStorage.getItem('redirectAfterLogin');
		
		const isAuthenticated = await authAPI.checkAuth();
		if (isAuthenticated) {
			// Already logged in, redirect based on user type or to redirectUrl
			const user = $authStore.user;
			
			if (redirectUrl) {
				sessionStorage.removeItem('redirectAfterLogin');
				await goto(redirectUrl, { replaceState: true });
			} else if (user?.user_type === 'student') {
				await goto(resolve('/student'), { replaceState: true });
			} else {
				await goto(resolve('/staff'), { replaceState: true });
			}
		}
		isCheckingAuth = false;
	});

	async function handleSubmit(e: Event) {
		e.preventDefault();
		isLoading = true;
		errorMessage = '';

		// Validate Thai national ID (13 digits)
		if (!/^\d{13}$/.test(nationalId)) {
			errorMessage = 'เลขบัตรประชาชนต้องเป็นตัวเลข 13 หลักเท่านั้น';
			isLoading = false;
			return;
		}

		try {
			const user = await authAPI.login({
				nationalId,
				password,
				rememberMe
			});

			// Clear redirectUrl from sessionStorage
			sessionStorage.removeItem('redirectAfterLogin');

			// Redirect to intended URL or default dashboard
			if (redirectUrl) {
				await goto(redirectUrl, { invalidateAll: true });
			} else if (user.user_type === 'student') {
				await goto(resolve('/student'), { invalidateAll: true });
			} else {
				await goto(resolve('/staff'), { invalidateAll: true });
			}
		} catch (error) {
			// Error already shown via toast in authAPI
			errorMessage = error instanceof Error ? error.message : 'เกิดข้อผิดพลาด';
		} finally {
			isLoading = false;
		}
	}

	function goBack() {
		window.location.href = '/';
	}

	function formatNationalId(value: string) {
		// Remove non-digits
		const digits = value.replace(/\D/g, '');
		// Limit to 13 digits
		return digits.slice(0, 13);
	}

	function handleNationalIdInput(e: Event) {
		const target = e.target as HTMLInputElement;
		nationalId = formatNationalId(target.value);
	}
</script>

<svelte:head>
	<title>เข้าสู่ระบบ - SchoolOrbit</title>
</svelte:head>

<div class="min-h-screen bg-background flex items-center justify-center p-4">
	{#if isCheckingAuth}
		<!-- Loading state while checking auth -->
		<div class="text-center">
			<div
				class="w-16 h-16 bg-primary rounded-lg flex items-center justify-center mx-auto mb-4 animate-pulse"
			>
				<GraduationCap class="w-8 h-8 text-primary-foreground" />
			</div>
			<p class="text-muted-foreground">กำลังตรวจสอบ...</p>
		</div>
	{:else}
		<div class="w-full max-w-md">
			<!-- Back Button -->
			<Button variant="ghost" onclick={goBack} class="mb-6">
				<ArrowLeft class="w-4 h-4 mr-2" />
				กลับหน้าหลัก
			</Button>

			<!-- Card -->
			<div class="bg-card border rounded-lg shadow-sm p-8">
				<!-- Logo & Title -->
				<div class="text-center mb-8">
					<div
						class="w-16 h-16 bg-primary rounded-lg flex items-center justify-center mx-auto mb-4"
					>
						<GraduationCap class="w-8 h-8 text-primary-foreground" />
					</div>
					<h1 class="text-2xl font-bold text-foreground mb-2">เข้าสู่ระบบ</h1>
					<p class="text-sm text-muted-foreground">SchoolOrbit - ระบบบริหารจัดการโรงเรียน</p>
				</div>

				<!-- Error Message -->
				{#if errorMessage}
					<div class="mb-6 p-3 bg-destructive/10 border border-destructive/20 rounded-lg">
						<p class="text-sm text-destructive text-center">{errorMessage}</p>
					</div>
				{/if}

				<!-- Login Form -->
				<form onsubmit={handleSubmit} class="space-y-6">
					<!-- National ID Input -->
					<div class="space-y-2">
						<Label for="nationalId">เลขบัตรประชาชน</Label>
						<Input
							type="text"
							id="nationalId"
							bind:value={nationalId}
							oninput={handleNationalIdInput}
							placeholder="1234567890123"
							maxlength={13}
							autocomplete="off"
							required
						/>
						<p class="text-xs text-muted-foreground">กรอกเลขบัตรประชาชน 13 หลัก</p>
					</div>

					<!-- Password Input -->
					<div class="space-y-2">
						<Label for="password">รหัสผ่าน</Label>
						<Input
							type="password"
							id="password"
							bind:value={password}
							placeholder="••••••••"
							autocomplete="current-password"
							required
						/>
					</div>

					<!-- Remember & Forgot -->
					<div class="flex items-center justify-between text-sm">
						<div class="flex items-center gap-2 cursor-pointer">
							<Checkbox
								checked={rememberMe}
								onCheckedChange={(checked) => (rememberMe = checked ?? false)}
							/>
							<span class="text-muted-foreground">จดจำฉันไว้</span>
						</div>
						<Button type="button" variant="link" class="p-0 h-auto text-sm"
							>ติดต่อผู้ดูแลระบบ</Button
						>
					</div>

					<!-- Submit Button -->
					<Button type="submit" class="w-full" disabled={isLoading}>
						{isLoading ? 'กำลังเข้าสู่ระบบ...' : 'เข้าสู่ระบบ'}
					</Button>
				</form>

				<!-- Info Section -->
				<div class="mt-6 pt-6 border-t border-border">
					<div class="text-center space-y-2">
						<p class="text-sm text-muted-foreground">ไม่มีการลงทะเบียนด้วยตนเอง</p>
						<p class="text-xs text-muted-foreground">บัญชีผู้ใช้จะถูกสร้างโดยผู้ดูแลระบบเท่านั้น</p>
					</div>
				</div>
			</div>
		</div>
	{/if}
</div>
