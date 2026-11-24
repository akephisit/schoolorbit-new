import { PERMISSIONS } from '#lib/permissions/registry.js';
import type { User } from '#lib/stores/auth.js';

export const assessmentReadPermissions = [
	PERMISSIONS.ACADEMIC_ASSESSMENT_READ_ASSIGNED,
	PERMISSIONS.ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT,
	PERMISSIONS.ACADEMIC_ASSESSMENT_READ_SCHOOL,
	PERMISSIONS.ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED,
	PERMISSIONS.ACADEMIC_ASSESSMENT_MANAGE_SCHOOL
] as const;

export function assessmentIdentityKey(
	user: User | null,
	sessionEpoch: number,
	hasPermission: (permission: string) => boolean
): string {
	if (!user) return '';
	const permissions = assessmentReadPermissions.filter(hasPermission).join(',');
	return `${sessionEpoch}:${user.user_type ?? ''}:${user.id}:${permissions}`;
}
