import { writable, derived } from 'svelte/store';
import type { Writable, Readable } from 'svelte/store';
import { PERMISSIONS, WILDCARD_PERMISSION } from '$lib/permissions/registry';

export const userPermissions: Writable<string[]> = writable([]);

export function hasPermission(permissions: string[], requiredPermission: string): boolean {
	return permissions.includes('*') || permissions.includes(requiredPermission);
}

export function hasAnyPermission(permissions: string[], requiredPermissions: string[]): boolean {
	return requiredPermissions.some((permission) => hasPermission(permissions, permission));
}

export function hasAllPermissions(permissions: string[], requiredPermissions: string[]): boolean {
	return requiredPermissions.every((permission) => hasPermission(permissions, permission));
}

export function hasModulePermission(permissions: string[], module: string): boolean {
	if (!module) return true;

	const modulePrefix = `${module}.`;
	return permissions.some(
		(permission) =>
			permission === '*' ||
			permission === module ||
			permission.startsWith(modulePrefix) ||
			permission.startsWith('*.')
	);
}

export function workflowManagePermissions(permissions: string[]): string[] {
	if (permissions.includes(WILDCARD_PERMISSION)) {
		return Object.values(PERMISSIONS).filter((permission) => permission.includes('.manage.'));
	}

	return permissions.filter((permission) => permission.includes('.manage.'));
}

export function hasWorkflowManagePermission(permissions: string[]): boolean {
	return workflowManagePermissions(permissions).length > 0;
}

/**
 * Permission checker for UI gating only. Backend remains the source of truth.
 */
export const can: Readable<{
	has: (permission: string) => boolean;
	hasModule: (module: string) => boolean;
	hasAny: (...permissions: string[]) => boolean;
	hasAll: (...permissions: string[]) => boolean;
	hasWorkflowManage: () => boolean;
}> = derived(userPermissions, ($permissions) => ({
	has: (permission: string): boolean => {
		return hasPermission($permissions, permission);
	},

	hasModule: (module: string): boolean => {
		return hasModulePermission($permissions, module);
	},

	hasAny: (...permissions: string[]): boolean => {
		return hasAnyPermission($permissions, permissions);
	},

	hasAll: (...permissions: string[]): boolean => {
		return hasAllPermissions($permissions, permissions);
	},

	hasWorkflowManage: (): boolean => {
		return hasWorkflowManagePermission($permissions);
	}
}));

export function setPermissions(permissions: string[] | undefined): void {
	if (permissions && Array.isArray(permissions)) {
		userPermissions.set(permissions);
	} else {
		userPermissions.set([]);
	}
}

export function clearPermissions(): void {
	userPermissions.set([]);
}
