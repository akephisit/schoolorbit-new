<script lang="ts">
	import { Settings, LogOut, UserCircle } from 'lucide-svelte';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { authStore } from '$lib/stores/auth';
	import { authAPI } from '$lib/api/auth';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';

	const user = $derived($authStore.user);
	const isLoading = $derived($authStore.isLoading);

	async function handleLogout() {
		await authAPI.logout();
		await goto(resolve('/login'), { invalidateAll: true });
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

	// Get profile and settings URLs based on user type
	function getProfileUrl(): string {
		if (!user) return resolve('/staff/profile');
		return user.user_type === 'student' ? resolve('/student/profile') : resolve('/staff/profile');
	}

	function getSettingsUrl(): string {
		if (!user) return resolve('/staff/settings');
		return user.user_type === 'student' ? resolve('/student/settings') : resolve('/staff/settings');
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
			class="flex items-center gap-3 px-2 py-2 rounded-lg hover:bg-accent transition-colors outline-none"
		>
			<!-- Avatar Only -->
			<!-- Avatar Or Initials -->
			{#if user.profileImageUrl}
				<img
					src={user.profileImageUrl}
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
				{#if user.email}
					<p class="text-xs text-muted-foreground mt-0.5 truncate">
						{user.email}
					</p>
				{/if}
			</div>

			<!-- Menu Items -->
			<DropdownMenu.Group>
				<DropdownMenu.Item class="cursor-pointer" onclick={() => goto(getProfileUrl())}>
					<UserCircle class="w-4 h-4 mr-2" />
					<span>โปรไฟล์ของฉัน</span>
				</DropdownMenu.Item>

				<DropdownMenu.Item class="cursor-pointer" onclick={() => goto(getSettingsUrl())}>
					<Settings class="w-4 h-4 mr-2" />
					<span>การตั้งค่า</span>
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
