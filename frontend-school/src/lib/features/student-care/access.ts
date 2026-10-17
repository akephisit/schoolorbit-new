import { derived, get } from 'svelte/store';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { authStore } from '#lib/stores/auth.js';
import { can, userPermissions } from '#lib/stores/permissions.js';

export const careIdentity = derived([authStore, userPermissions], () => appIdentityKey());
export type CareRead<T> = { identity: string; resource: T | null };

export async function readCare<T>(
	permissions: string[],
	read: () => Promise<T>
): Promise<CareRead<T>> {
	const user = await waitForAuthenticatedUser();
	const identity = appIdentityKey();
	return { identity, resource: user && get(can).hasAny(...permissions) ? await read() : null };
}

import { PERMISSIONS } from '#lib/permissions/registry.js';
export const careRoutineReadPermissions = [
	PERMISSIONS.STUDENT_CARE_READ_ASSIGNED,
	PERMISSIONS.STUDENT_CARE_READ_SCHOOL,
	PERMISSIONS.STUDENT_CARE_MANAGE_ASSIGNED,
	PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL
];
export const careManagePermissions = [
	PERMISSIONS.STUDENT_CARE_MANAGE_ASSIGNED,
	PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL
];
export const careSensitiveReadPermissions = [
	PERMISSIONS.STUDENT_CARE_SENSITIVE_READ_ASSIGNED,
	PERMISSIONS.STUDENT_CARE_SENSITIVE_READ_SCHOOL,
	PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_ASSIGNED,
	PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL
];
export const careSensitiveManagePermissions = [
	PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_ASSIGNED,
	PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL
];

export const careReadPermissions = [
	...careRoutineReadPermissions,
	PERMISSIONS.STUDENT_CARE_READ_OWN,
	PERMISSIONS.STUDENT_CARE_CONFIG_MANAGE_SCHOOL,
	...careSensitiveReadPermissions
];

import type { CareAssignment, CareCase, CareVisit } from '#lib/api/student-care.js';
export function mayManageCareAssignment(assignment: CareAssignment): boolean {
	const permissions = get(can),
		actor = get(authStore).user?.id;
	return (
		permissions.has(PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL) ||
		!!(
			actor &&
			permissions.has(PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_ASSIGNED) &&
			assignment.reviewerUserIds.includes(actor)
		)
	);
}
export function mayManageCareCase(record: CareCase): boolean {
	const permissions = get(can),
		actor = get(authStore).user?.id;
	return (
		permissions.has(PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL) ||
		!!(
			actor &&
			permissions.has(PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_ASSIGNED) &&
			record.assignedUserIds.includes(actor)
		)
	);
}
export function mayManageCareVisit(visit: CareVisit): boolean {
	const permissions = get(can),
		actor = get(authStore).user?.id;
	return (
		permissions.has(PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL) ||
		!!(
			actor &&
			permissions.has(PERMISSIONS.STUDENT_CARE_MANAGE_ASSIGNED) &&
			visit.visitorIds.includes(actor)
		)
	);
}
export function mayReadCareAssignment(assignment: CareAssignment): boolean {
	const permissions = get(can),
		actor = get(authStore).user?.id;
	return (
		permissions.hasAny(
			PERMISSIONS.STUDENT_CARE_SENSITIVE_READ_SCHOOL,
			PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL
		) ||
		!!(
			actor &&
			((permissions.has(PERMISSIONS.STUDENT_CARE_READ_OWN) && assignment.respondentId === actor) ||
				(permissions.hasAny(
					PERMISSIONS.STUDENT_CARE_SENSITIVE_READ_ASSIGNED,
					PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_ASSIGNED
				) &&
					assignment.reviewerUserIds.includes(actor)))
		)
	);
}
export function mayCancelCareAssignment(assignment: CareAssignment): boolean {
	const permissions = get(can),
		actor = get(authStore).user?.id;
	return (
		assignment.status !== 'submitted' &&
		assignment.status !== 'cancelled' &&
		(permissions.has(PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL) ||
			!!(
				actor &&
				permissions.has(PERMISSIONS.STUDENT_CARE_MANAGE_ASSIGNED) &&
				assignment.createdBy === actor
			))
	);
}
