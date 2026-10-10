import { derived, get } from 'svelte/store';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { authStore } from '#lib/stores/auth.js';
import { can, userPermissions } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { currentAttendanceDate } from '#lib/api/attendance.js';

export const attendanceIdentity = derived([authStore, userPermissions], () => appIdentityKey());
export type AttendanceRead<T> = { identityKey: string; resource: T | null; error: string | null };
export async function captureAttendanceRead<T>(
	identityKey: string,
	operation: Promise<T | null>,
	fallback: string
): Promise<AttendanceRead<T>> {
	const result = await captureRouteLoad(operation, fallback);
	return { identityKey, resource: result.ok ? result.data : null, error: result.error };
}
export async function readAttendance<T>(
	permissions: string[],
	read: () => Promise<T>,
	userType: 'staff' | 'parent' | 'student' = 'staff'
): Promise<AttendanceRead<T>> {
	const user = await waitForAuthenticatedUser();
	const identityKey = appIdentityKey();
	return captureAttendanceRead(
		identityKey,
		user?.user_type === userType && get(can).hasAny(...permissions)
			? read()
			: Promise.resolve(null),
		'โหลดข้อมูลเช็คชื่อไม่ได้'
	);
}
export function resolveAttendanceDate(value: string | null): string {
	if (value && /^\d{4}-\d{2}-\d{2}$/.test(value)) {
		const parsed = new Date(value + 'T00:00:00Z');
		if (Number.isFinite(parsed.getTime()) && parsed.toISOString().slice(0, 10) === value)
			return value;
	}
	return currentAttendanceDate();
}

export const ATTENDANCE_FACE_PERMISSIONS = [
	PERMISSIONS.ATTENDANCE_ENROLL_ASSIGNED,
	PERMISSIONS.ATTENDANCE_ENROLL_SCHOOL,
	PERMISSIONS.ATTENDANCE_VERIFY_ASSIGNED
];

export const ATTENDANCE_STAFF_PERMISSIONS = [
	PERMISSIONS.ATTENDANCE_READ_ASSIGNED,
	PERMISSIONS.ATTENDANCE_READ_SCHOOL,
	PERMISSIONS.ATTENDANCE_UPDATE_ASSIGNED,
	PERMISSIONS.ATTENDANCE_UPDATE_SCHOOL,
	PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL
];
