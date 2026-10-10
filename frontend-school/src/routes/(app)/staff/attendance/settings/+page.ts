import type { PageLoad } from './$types';
import { attendanceSettings, attendanceOptions, attendanceDays } from '#lib/api/attendance.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import {
	readAttendance,
	resolveAttendanceDate
} from '#lib/features/attendance/attendance-access.js';
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
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const term = url.searchParams.get('academicTermId');
	const date = resolveAttendanceDate(url.searchParams.get('date'));
	const monthStart = date.slice(0, 7) + '-01';
	const monthEnd = new Date(Date.UTC(Number(date.slice(0, 4)), Number(date.slice(5, 7)), 0))
		.toISOString()
		.slice(0, 10);
	return {
		title: _meta.menu.title,
		term,
		date,
		settings: term
			? captureRouteLoad(
					readAttendance([PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL], () =>
						Promise.all([
							attendanceSettings(term, { requestFetch: fetch }),
							attendanceDays(term, monthStart, monthEnd, { requestFetch: fetch })
						])
					),
					'โหลดข้อมูลเช็คชื่อไม่ได้'
				)
			: null,
		options: term
			? captureRouteLoad(
					readAttendance([PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL], () =>
						attendanceOptions(term, { requestFetch: fetch })
					),
					'โหลดกลุ่มและเครื่องสแกนไม่ได้'
				)
			: null
	};
};
