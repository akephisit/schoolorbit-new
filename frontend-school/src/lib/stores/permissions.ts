/**
 * Enhanced Permission Store
 * - Auto-syncs permissions from authStore.user
 * - Provides easy-to-use API via `can` store
 * - No separate API call needed
 */
import { writable, derived, get } from 'svelte/store';
import type { Writable, Readable } from 'svelte/store';

// Internal state
export const userPermissions: Writable<string[]> = writable([]);
export const permissionsLoading: Writable<boolean> = writable(false);

/**
 * Enhanced permission checker with multiple methods
 * Usage: $can.has('permission'), $can.hasAny(...), etc.
 */
export const can: Readable<{
	has: (permission: string) => boolean;
	hasScoped: (resource: string, action: string, scope: 'own' | 'all') => boolean;
	hasAny: (...permissions: string[]) => boolean;
	hasAll: (...permissions: string[]) => boolean;
}> = derived(userPermissions, ($permissions) => ({
	/**
	 * Check if user has exact permission
	 * @example can.has('achievement.create.own') // true/false
	 */
	has: (permission: string): boolean => {
		if ($permissions.includes('*')) return true; // Admin wildcard
		return $permissions.includes(permission);
	},

	/**
	 * Check scoped permission with hierarchy support
	 * Hierarchy: .all > .own
	 * User with .all can access .own
	 * @example can.hasScoped('achievement', 'create', 'own')
	 */
	hasScoped: (resource: string, action: string, scope: 'own' | 'all'): boolean => {
		const required = `${resource}.${action}.${scope}`;
		if ($permissions.includes('*')) return true;

		// Exact match
		if ($permissions.includes(required)) return true;

		// Hierarchy: .all includes .own
		if (scope === 'own' && $permissions.includes(`${resource}.${action}.all`)) {
			return true;
		}

		return false;
	},

	/**
	 * Check if user has ANY of the permissions (OR)
	 * @example can.hasAny('achievement.create.own', 'achievement.create.all')
	 */
	hasAny: (...permissions: string[]): boolean => {
		if ($permissions.includes('*')) return true;
		return permissions.some((p) => $permissions.includes(p));
	},

	/**
	 * Check if user has ALL of the permissions (AND)
	 * @example can.hasAll('achievement.create', 'achievement.read')
	 */
	hasAll: (...permissions: string[]): boolean => {
		if ($permissions.includes('*')) return true;
		return permissions.every((p) => $permissions.includes(p));
	}
}));

/**
 * Load permissions from user object (from /api/auth/me response)
 * This avoids 403 error from /api/users/{id}/permissions which requires admin permission
 * @param permissions - Permissions array from user object
 */
export function setPermissions(permissions: string[] | undefined): void {
	if (permissions && Array.isArray(permissions)) {
		userPermissions.set(permissions);
	} else {
		userPermissions.set([]);
	}
}

/**
 * Legacy function for backward compatibility
 * Now just extracts permissions from authStore
 * @deprecated Use setPermissions() with user.permissions instead
 */
export async function loadUserPermissions(userId: string, force = false): Promise<void> {
	// This function is deprecated but kept for compatibility
	// Permissions should come from authStore.user.permissions
	// which is populated by /api/auth/me

	// Try to get permissions from separate API (will fail with 403 for non-admin)
	permissionsLoading.set(true);
	try {
		const { userRoleAPI } = await import('$lib/api/roles');
		const response = await userRoleAPI.getUserPermissions(userId);
		if (response.success && response.data) {
			userPermissions.set(response.data);
		} else {
			console.warn(
				'Cannot load permissions via API (requires admin permission). Will use permissions from auth/me instead.'
			);
			userPermissions.set([]);
		}
	} catch (error) {
		console.warn('Failed to load permissions via API:', error);
		userPermissions.set([]);
	} finally {
		permissionsLoading.set(false);
	}
}

/**
 * Clear permissions (call on logout)
 */
export function clearPermissions(): void {
	userPermissions.set([]);
}

/**
 * Refresh permissions from current auth state
 */
export function refreshPermissions(): void {
	// Permissions will be refreshed when user re-authenticates
	// via authStore.setUser() which calls setPermissions()
}
