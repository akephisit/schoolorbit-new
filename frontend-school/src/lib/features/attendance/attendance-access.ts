import { get } from 'svelte/store';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';

export const ATTENDANCE_FACE_PERMISSIONS = [
	PERMISSIONS.ATTENDANCE_ENROLL_ASSIGNED,
	PERMISSIONS.ATTENDANCE_ENROLL_SCHOOL,
	PERMISSIONS.ATTENDANCE_VERIFY_ASSIGNED
];
export async function waitForAttendanceAccess(permissions: string[]): Promise<boolean> {
	const user = await waitForAuthenticatedUser();
	return user?.user_type === 'staff' && get(can).hasAny(...permissions);
}
