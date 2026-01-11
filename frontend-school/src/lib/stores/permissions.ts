/**
 * Enhanced Permission Store
 * - Auto-loads permissions when user logs in
 * - Provides easy-to-use API via `can` store
 * - No need to use utils - everything in one place
 */
import { writable, derived, get } from 'svelte/store';
import type { Writable, Readable } from 'svelte/store';

// Internal state
export const userPermissions: Writable<string[]> = writable([]);
export const permissionsLoading: Writable<boolean> = writable(false);
let lastLoadedUserId: string | null = null;

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
 * Load permissions for current user
 * @param userId - User ID to load permissions for
 * @param force - Force reload even if already loaded for this user
 */
export async function loadUserPermissions(userId: string, force = false): Promise<void> {
	// Skip if already loaded for this user (unless forced)
	if (!force && lastLoadedUserId === userId && get(userPermissions).length > 0) {
		return;
	}

	permissionsLoading.set(true);
	try {
		const { userRoleAPI } = await import('$lib/api/roles');
		const response = await userRoleAPI.getUserPermissions(userId);
		if (response.success && response.data) {
			userPermissions.set(response.data);
			lastLoadedUserId = userId;
		} else {
			console.error('Failed to load permissions:', response.error);
			userPermissions.set([]);
			lastLoadedUserId = null;
		}
	} catch (error) {
		console.error('Error loading permissions:', error);
		userPermissions.set([]);
		lastLoadedUserId = null;
	} finally {
		permissionsLoading.set(false);
	}
}

/**
 * Clear permissions (call on logout)
 */
export function clearPermissions(): void {
	userPermissions.set([]);
	lastLoadedUserId = null;
}

/**
 * Refresh permissions for current user
 */
export async function refreshPermissions(): Promise<void> {
	if (lastLoadedUserId) {
		await loadUserPermissions(lastLoadedUserId, true);
	}
}
