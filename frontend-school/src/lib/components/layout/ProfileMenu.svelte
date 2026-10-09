<script lang="ts">
	import { Settings, LogOut, ShieldCheck, UserCircle } from '@lucide/svelte';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { authStore } from '#lib/stores/auth.js';
	import { authAPI } from '#lib/api/auth.js';
	import { goto, preloadData } from '$app/navigation';
	import { resolve } from '$app/paths';
	import PrivateFileImage from '#lib/components/files/PrivateFileImage.svelte';

	const user = $derived($authStore.user);
	const isLoading = $derived($authStore.isLoading);

	async function handleLogout() {
		const epoch = authStore.sessionEpoch;
		await authAPI.logout();
		if ($authStore.isAuthenticated || authStore.sessionEpoch !== epoch + 1) return;
		// Clear redirectAfterLogin to prevent layout from redirecting back to protected page
		sessionStorage.removeItem('redirectAfterLogin');
		await goto(resolve('/'), { refreshAll: true });
	}

	// Get initials from first and last name
	function getInitials(firstName?: string, lastName?: string): string {
		if (!firstName || !lastName) return 'U';
		return `${firstName.charAt(0)}${lastName.charAt(0)}`.toUpperCase();
	}

	// Get display role from database
	function getDisplayRole(): string {
		if (!user) return 'ผู้ใช้งาน';

		// Use primaryRoleName from backend (from roles table)
		return user.primaryRoleName || 'ผู้ใช้งาน';
	}

	// Navigate to profile based on user type
	function goToProfile() {
		if (!user || user.user_type === 'student') {
			goto(resolve('student/profile'));
		} else {
			goto(resolve('staff/profile'));
		}
	}

	// Navigate to settings based on user type
	function goToSettings() {
		if (!user || user.user_type === 'student') {
			goto(resolve('student/settings'));
		} else {
			goto(resolve('staff/settings'));
		}
	}

	async function goToSecurity() {
		const epoch = authStore.sessionEpoch;
		const target = resolve('account/security');
		await preloadData(target);
		if (!$authStore.isAuthenticated || authStore.sessionEpoch !== epoch) return;
		await goto(target);
	}
</script>

{#if isLoading}
	<!-- Loading Skeleton -->
	<div class="flex items-center gap-3 px-2 py-2">
		<div
			class="w-10 h-10 rounded-full bg-gradient-to-br from-primary/20 to-primary/10 flex items-center justify-center flex-shrink-0 animate-pulse ring-2 ring-background"
		>
			<UserCircle class="w-6 h-6 text-muted-foreground/50" />
		</div>
	</div>
{:else if user}
	<DropdownMenu.Root>
		<DropdownMenu.Trigger
			aria-label="เปิดเมนูบัญชี"
			class="flex items-center gap-3 px-2 py-2 rounded-lg hover:bg-accent transition-colors outline-none"
		>
			<!-- Avatar Only -->
			<!-- Avatar Or Initials -->
			{#if user.profileImageFileId}
				<PrivateFileImage
					fileId={user.profileImageFileId}
					resourceId={user.id}
					alt="Profile"
					class="w-10 h-10 rounded-full object-cover shadow-sm ring-2 ring-background bg-muted"
				/>
			{:else}
				<div
					class="w-10 h-10 rounded-full bg-gradient-to-br from-primary to-primary/80 flex items-center justify-center flex-shrink-0 shadow-sm ring-2 ring-background"
				>
					<span class="text-sm font-semibold text-primary-foreground">
						{getInitials(user.firstName, user.lastName)}
					</span>
				</div>
			{/if}
		</DropdownMenu.Trigger>

		<DropdownMenu.Content align="end" side="bottom" class="w-56">
			<!-- User Info Section -->
			<div class="px-2 py-2 border-b border-border">
				<p class="text-sm font-semibold text-foreground">
					{user.firstName}
					{user.lastName}
				</p>
				<p class="text-xs text-muted-foreground mt-0.5">
					{getDisplayRole()}
				</p>
			</div>

			<!-- Menu Items -->
			<DropdownMenu.Group>
				<DropdownMenu.Item class="cursor-pointer" onclick={goToProfile}>
					<UserCircle class="w-4 h-4 mr-2" />
					<span>โปรไฟล์ของฉัน</span>
				</DropdownMenu.Item>

				<DropdownMenu.Item class="cursor-pointer" onclick={goToSettings}>
					<Settings class="w-4 h-4 mr-2" />
					<span>การตั้งค่า</span>
				</DropdownMenu.Item>

				<DropdownMenu.Item class="cursor-pointer" onclick={goToSecurity}>
					<ShieldCheck class="w-4 h-4 mr-2" />
					<span>ความปลอดภัยของบัญชี</span>
				</DropdownMenu.Item>
			</DropdownMenu.Group>

			<DropdownMenu.Separator />

			<!-- Logout -->
			<DropdownMenu.Item
				onclick={handleLogout}
				class="cursor-pointer text-destructive focus:text-destructive focus:bg-destructive/10"
			>
				<LogOut class="w-4 h-4 mr-2" />
				<span>ออกจากระบบ</span>
			</DropdownMenu.Item>
		</DropdownMenu.Content>
	</DropdownMenu.Root>
{/if}
