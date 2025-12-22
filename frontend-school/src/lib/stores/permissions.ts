// Permission store for global access
import { writable, derived } from 'svelte/store';
import type { Writable, Readable } from 'svelte/store';

export const userPermissions: Writable<string[]> = writable([]);
export const permissionsLoading: Writable<boolean> = writable(false);

// Helper to check if user has permission
export const hasPermission: Readable<(permission: string) => boolean> = derived(
	userPermissions,
	($permissions) => (permission: string) => {
		// Admin wildcard - has all permissions
		if ($permissions.includes('*')) return true;
		// Check specific permission
		return $permissions.includes(permission);
	}
);

// Load permissions for current user
export async function loadUserPermissions(userId: string): Promise<void> {
	permissionsLoading.set(true);
	try {
		const { userRoleAPI } = await import('$lib/api/roles');
		const response = await userRoleAPI.getUserPermissions(userId);
		if (response.success && response.data) {
			userPermissions.set(response.data);
		} else {
			console.error('Failed to load permissions:', response.error);
			userPermissions.set([]);
		}
	} catch (error) {
		console.error('Error loading permissions:', error);
		userPermissions.set([]);
	} finally {
		permissionsLoading.set(false);
	}
}

// Clear permissions (on logout)
export function clearPermissions(): void {
	userPermissions.set([]);
}
