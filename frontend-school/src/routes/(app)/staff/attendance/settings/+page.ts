import type { PageLoad } from './$types';
import {
	currentAttendanceDate,
	attendanceSettings,
	attendanceOptions,
	attendanceDays
} from '#lib/api/attendance.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { waitForAttendanceAccess } from '#lib/features/attendance/attendance-access.js';
export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'ตั้งค่าเช็คชื่อ',
		icon: 'CalendarDays',
		group: 'academic_delivery',
		workspace: 'academic',
		permission: PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL,
		order: 41,
		user_type: 'staff'
	}
};
export const load: PageLoad = ({ fetch, url }) => {
	const term = url.searchParams.get('academicTermId');
	const date = url.searchParams.get('date') || currentAttendanceDate();
	const monthStart = date.slice(0, 7) + '-01';
	const monthEnd = new Date(Date.UTC(Number(date.slice(0, 4)), Number(date.slice(5, 7)), 0))
		.toISOString()
		.slice(0, 10);
	const access = waitForAttendanceAccess([PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL]);
	return {
		title: _meta.menu.title,
		term,
		date,
		settings: term
			? captureRouteLoad(
					access.then((allowed) =>
						allowed
							? Promise.all([
									attendanceSettings(term, { requestFetch: fetch }),
									attendanceDays(term, monthStart, monthEnd, { requestFetch: fetch })
								])
							: null
					),
					'โหลดข้อมูลเช็คชื่อไม่ได้'
				)
			: null,
		options: term
			? captureRouteLoad(
					access.then((allowed) =>
						allowed ? attendanceOptions(term, { requestFetch: fetch }) : null
					),
					'โหลดกลุ่มและเครื่องสแกนไม่ได้'
				)
			: null
	};
};
