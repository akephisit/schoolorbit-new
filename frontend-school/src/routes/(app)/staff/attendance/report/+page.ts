import type { PageLoad } from './$types';
import { attendanceReport } from '#lib/api/attendance.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import {
	readAttendance,
	resolveAttendanceDate
} from '#lib/features/attendance/attendance-access.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'สรุปการเช็คชื่อ',
		icon: 'ChartColumn',
		group: 'academic_delivery',
		workspace: 'academic',
		permission: [
			PERMISSIONS.ATTENDANCE_READ_ASSIGNED,
			PERMISSIONS.ATTENDANCE_READ_SCHOOL,
			PERMISSIONS.ATTENDANCE_UPDATE_ASSIGNED,
			PERMISSIONS.ATTENDANCE_UPDATE_SCHOOL,
			PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL
		],
		order: 43,
		user_type: 'staff'
	}
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const term = url.searchParams.get('academicTermId');
	const date = resolveAttendanceDate(url.searchParams.get('date'));
	return {
		title: _meta.menu.title,
		term,
		date,
		initial: term
			? captureRouteLoad(
					readAttendance(_meta.menu.permission, () =>
						attendanceReport(term, undefined, { requestFetch: fetch })
					),
					'โหลดข้อมูลเช็คชื่อไม่ได้'
				)
			: null
	};
};
