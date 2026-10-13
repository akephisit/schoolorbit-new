import { derived, get } from 'svelte/store';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { authStore } from '#lib/stores/auth.js';
import { can, userPermissions } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
export const behaviorIdentity = derived([authStore, userPermissions], () => appIdentityKey());
export const behaviorReadPermissions = [
	PERMISSIONS.BEHAVIOR_READ_OWN,
	PERMISSIONS.BEHAVIOR_READ_ASSIGNED,
	PERMISSIONS.BEHAVIOR_READ_SCHOOL
];
export const behaviorCreatePermissions = [
	PERMISSIONS.BEHAVIOR_CREATE_ASSIGNED,
	PERMISSIONS.BEHAVIOR_CREATE_SCHOOL
];
export type BehaviorRead<T> = { identity: string; resource: T | null };
export async function readBehavior<T>(
	permissions: string[],
	read: () => Promise<T>
): Promise<BehaviorRead<T>> {
	const user = await waitForAuthenticatedUser();
	const identity = appIdentityKey();
	return { identity, resource: user && get(can).hasAny(...permissions) ? await read() : null };
}
