<script lang="ts">
	import { Settings, LogOut, ChevronDown, UserCircle } from 'lucide-svelte';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { authStore } from '$lib/stores/auth';
	import { authAPI } from '$lib/api/auth';
	import { goto } from '$app/navigation';

	interface Props {
		collapsed?: boolean;
	}

	let { collapsed = false }: Props = $props();

	const user = $derived($authStore.user);

	async function handleLogout() {
		await authAPI.logout();
		await goto('/login', { invalidateAll: true });
	}

	// Get initials from first and last name
	function getInitials(firstName?: string, lastName?: string): string {
		if (!firstName || !lastName) return 'U';
		return `${firstName.charAt(0)}${lastName.charAt(0)}`.toUpperCase();
	}

	// Get display role (primary role)
	function getDisplayRole(role?: string): string {
		if (!role) return 'ผู้ใช้งาน';
		
		// Map role to Thai display name
		const roleMap: Record<string, string> = {
			admin: 'ผู้ดูแลระบบ',
			teacher: 'ครู',
			staff: 'เจ้าหน้าที่',
			student: 'นักเรียน'
		};
		return roleMap[role.toLowerCase()] || role;
	}
</script>

{#if user}
	<DropdownMenu.Root>
		<DropdownMenu.Trigger
			class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-accent transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 w-full {collapsed
				? 'justify-center'
				: ''}"
		>
			<!-- Avatar -->
			<div
				class="w-10 h-10 rounded-full bg-gradient-to-br from-primary to-primary/80 flex items-center justify-center flex-shrink-0 shadow-sm ring-2 ring-background"
			>
				<span class="text-sm font-semibold text-primary-foreground">
					{getInitials(user.firstName, user.lastName)}
				</span>
			</div>

			{#if !collapsed}
				<!-- User Info -->
				<div class="flex-1 text-left min-w-0">
					<p class="text-sm font-semibold text-foreground truncate">
						{user.firstName}
						{user.lastName}
					</p>
					<p class="text-xs text-muted-foreground truncate">
						{getDisplayRole(user.role)}
					</p>
				</div>

				<!-- Chevron Icon -->
				<ChevronDown class="w-4 h-4 text-muted-foreground flex-shrink-0" />
			{/if}
		</DropdownMenu.Trigger>

		<DropdownMenu.Content
			align={collapsed ? 'start' : 'end'}
			side={collapsed ? 'right' : 'bottom'}
			class="w-56"
		>
			<!-- User Info Section -->
			<div class="px-2 py-2 border-b border-border">
				<p class="text-sm font-semibold text-foreground">
					{user.firstName}
					{user.lastName}
				</p>
				<p class="text-xs text-muted-foreground mt-0.5">
					{getDisplayRole(user.role)}
				</p>
				{#if user.email}
					<p class="text-xs text-muted-foreground mt-0.5 truncate">
						{user.email}
					</p>
				{/if}
			</div>

			<!-- Menu Items -->
			<DropdownMenu.Group>
				<DropdownMenu.Item class="cursor-pointer">
					<UserCircle class="w-4 h-4 mr-2" />
					<span>โปรไฟล์ของฉัน</span>
				</DropdownMenu.Item>

				<DropdownMenu.Item class="cursor-pointer">
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
