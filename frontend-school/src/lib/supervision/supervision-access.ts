import { PERMISSIONS } from '#lib/permissions/registry.js';
import { ApiClientError } from '#lib/api/client.js';
import { get } from 'svelte/store';
import { authStore } from '#lib/stores/auth.js';
import { can } from '#lib/stores/permissions.js';

export function isSupervisionDeniedOrMissing(error: unknown): boolean {
	return error instanceof ApiClientError && [401, 403, 404].includes(error.status);
}

export async function resolveSupervisionRead<T>(operation: Promise<T>): Promise<T | null> {
	try {
		return await operation;
	} catch (error) {
		if (isSupervisionDeniedOrMissing(error)) return null;
		throw error;
	}
}

export function waitForSupervisionAccess(permissions: string[] | string): Promise<boolean> {
	const allowed = () =>
		get(authStore).isAuthenticated &&
		(typeof permissions === 'string'
			? get(can).hasModule(permissions)
			: get(can).hasAny(...permissions));
	if (!get(authStore).isLoading) return Promise.resolve(allowed());
	return new Promise((resolve) => {
		const unsubscribe = authStore.subscribe((state) => {
			if (state.isLoading) return;
			queueMicrotask(() => {
				unsubscribe();
				resolve(allowed());
			});
		});
	});
}

export const SUPERVISION_OBSERVATION_READ_PERMISSIONS = [
	PERMISSIONS.SUPERVISION_READ_OWN,
	PERMISSIONS.SUPERVISION_READ_ASSIGNED,
	PERMISSIONS.SUPERVISION_READ_ORGANIZATION_UNIT,
	PERMISSIONS.SUPERVISION_READ_ORGANIZATION_TREE,
	PERMISSIONS.SUPERVISION_READ_SCHOOL,
	PERMISSIONS.SUPERVISION_MANAGE_SCHOOL,
	PERMISSIONS.SUPERVISION_MANAGE_ORGANIZATION_UNIT,
	PERMISSIONS.SUPERVISION_MANAGE_ORGANIZATION_TREE
];

export const SUPERVISION_SCHOOL_REPORT_PERMISSIONS = [
	PERMISSIONS.SUPERVISION_READ_SCHOOL,
	PERMISSIONS.SUPERVISION_MANAGE_SCHOOL
];
