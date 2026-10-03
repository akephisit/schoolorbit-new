<script lang="ts">
	import type { PageProps } from './$types';
	import PublicSchoolLogo from '$lib/components/school-public/PublicSchoolLogo.svelte';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { AuthCheckingState } from '$lib/components/app-state';
	import { ArrowLeft } from '@lucide/svelte';
	import { authAPI } from '$lib/api/auth';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';

	import { authStore } from '$lib/stores/auth';
	let { data }: PageProps = $props();

	let username = $state('');
	let password = $state('');
	let rememberMe = $state(false);
	let isLoading = $state(false);
	let isCheckingAuth = $state(true);
	let redirectUrl = $state<string | null>(null);

	// Check if user is already authenticated
	onMount(async () => {
		// Get redirect URL from sessionStorage (set by route guards)
		const storedRedirectUrl = sessionStorage.getItem('redirectAfterLogin');
		redirectUrl =
			storedRedirectUrl?.startsWith('/') &&
			!storedRedirectUrl.startsWith('//') &&
			storedRedirectUrl[1] !== '\\'
				? storedRedirectUrl
				: null;

		// Check if we just logged out using a simple flag (optional, but good for UX to skip check)
		// Or we can just let it fail quietly. But strictly speaking, if we just came here,
		// we likely don't have a session.
		// However, to fix the specific "401" log spam, we can't easily know "we just logged out"
		// unless we pass state.

		// In standard SPA, checking auth on mount of login page is correct behavior
		// (in case user manually navigated here but is already logged in).

		// To silence the 401 error log specifically for this case, we would need to know context.
		// For now, let's keep it as is, because that 401 *is* valid information (User is not logged in).
		// The user complained about it appearing "when logging out".

		// Let's modify valid check logic:
		const result = await authAPI.refreshCurrentUser({ silent: false });
		if (result === 'authenticated') {
			// Already logged in, redirect based on user type or to redirectUrl
			const user = $authStore.user;

			if (redirectUrl) {
				sessionStorage.removeItem('redirectAfterLogin');
				window.location.replace(redirectUrl);
				return;
			} else if (user?.user_type === 'parent') {
				await goto(resolve('/parent'), { replaceState: true });
			} else if (user?.user_type === 'student') {
				await goto(resolve('/student'), { replaceState: true });
			} else {
				await goto(resolve('/staff'), { replaceState: true });
			}
		} else if (result === 'unavailable') {
			toast.warning('ระบบยืนยันตัวตนไม่พร้อมใช้งาน กรุณาลองใหม่อีกครั้ง');
		}
		isCheckingAuth = false;
	});

	async function handleSubmit(e: Event) {
		e.preventDefault();
		isLoading = true;

		// Username validation (if any specific rule is needed, add here).
		// For now, just non-empty.
		if (!username.trim()) {
			toast.error('กรุณากรอกชื่อผู้ใช้งาน');
			isLoading = false;
			return;
		}

		try {
			// Note: authAPI.login needs to be updated to accept username instead of nationalId
			// Assuming we will update valid `login` in $lib/api/auth as a next step.
			// But here we construct the object that matches what backend expects if we use the same old function name
			// OR we update the type definition in the next step.
			// Let's assume authAPI.login will change signature.
			const user = await authAPI.login({
				username, // Changed from nationalId
				password,
				rememberMe
			});

			// Clear redirectUrl from sessionStorage
			sessionStorage.removeItem('redirectAfterLogin');

			// Redirect to intended URL or default dashboard
			if (redirectUrl) {
				window.location.assign(redirectUrl);
				return;
			} else if (user.user_type === 'parent') {
				await goto(resolve('/parent'), { invalidateAll: true });
			} else if (user.user_type === 'student') {
				await goto(resolve('/student'), { invalidateAll: true });
			} else {
				await goto(resolve('/staff'), { invalidateAll: true });
			}
		} catch (error) {
			// Error already shown via toast in authAPI
			const msg = error instanceof Error ? error.message : 'เกิดข้อผิดพลาด';
			toast.error(msg);
		} finally {
			isLoading = false;
		}
	}
</script>

<svelte:head>
	<title>เข้าสู่ระบบ - SchoolOrbit</title>
</svelte:head>

{#if isCheckingAuth}
	<AuthCheckingState message="กำลังตรวจสอบสิทธิ์..." />
{:else}
	<main class="login-page min-h-dvh bg-background flex flex-col items-center p-4">
		<div class="my-auto w-full max-w-md">
			<!-- Back Button -->
			<Button variant="ghost" href="/" class="mb-3 text-foreground sm:mb-6">
				<ArrowLeft class="w-4 h-4 mr-2" />
				กลับหน้าหลัก
			</Button>

			<!-- Card -->
			<div class="bg-card border border-border rounded-xl shadow-sm p-5 sm:p-8">
				<!-- Logo & Title -->
				<div class="text-center mb-4 sm:mb-6">
					<div class="mx-auto mb-2 h-28 w-40 sm:mb-3 sm:h-32" data-testid="login-school-crest">
						{#await data.identity}
							<Skeleton class="size-full" />
						{:then result}
							<PublicSchoolLogo
								fileId={result.ok ? result.data.logoFileId : null}
								class="size-full object-contain"
							/>
						{/await}
					</div>
					<h1 class="text-2xl font-bold text-foreground mb-1">เข้าสู่ระบบ</h1>
					{#await data.identity}
						<Skeleton class="mx-auto h-5 w-48 max-w-full" />
					{:then result}
						<p class="break-words text-sm text-muted-foreground">
							{result.ok && result.data.schoolName
								? result.data.schoolName
								: 'SchoolOrbit - ระบบบริหารจัดการโรงเรียน'}
						</p>
					{/await}
				</div>

				<!-- Login Form -->
				<form onsubmit={handleSubmit} class="space-y-4 sm:space-y-6">
					<!-- Username Input -->
					<div class="space-y-2">
						<Label for="username">ชื่อผู้ใช้งาน (Username)</Label>
						<Input
							type="text"
							id="username"
							bind:value={username}
							placeholder="เช่น T670001, 66001"
							autocomplete="username"
							required
						/>
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
					<div class="flex flex-wrap items-center justify-between gap-3 text-sm">
						<div class="flex items-center gap-2 cursor-pointer">
							<Checkbox
								id="remember-me"
								checked={rememberMe}
								onCheckedChange={(checked) => (rememberMe = checked ?? false)}
							/>
							<Label for="remember-me" class="cursor-pointer font-normal text-muted-foreground"
								>จดจำฉันไว้</Label
							>
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
				<div class="mt-4 pt-4 border-t border-border sm:mt-6 sm:pt-6">
					<div class="text-center space-y-1">
						<p class="text-xs text-muted-foreground">ไม่มีการลงทะเบียนด้วยตนเอง</p>
						<p class="text-xs text-muted-foreground">บัญชีผู้ใช้จะถูกสร้างโดยผู้ดูแลระบบเท่านั้น</p>
					</div>
				</div>
			</div>
		</div>
	</main>
{/if}
