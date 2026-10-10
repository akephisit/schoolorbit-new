import type { PageLoad } from './$types';
import { currentAttendanceDate, attendanceReport } from '#lib/api/attendance.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { waitForAttendanceAccess } from '#lib/features/attendance/attendance-access.js';
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
export const load: PageLoad = ({ fetch, url }) => {
	const term = url.searchParams.get('academicTermId');
	const date = url.searchParams.get('date') || currentAttendanceDate();
	return {
		title: _meta.menu.title,
		term,
		date,
		initial: term
			? captureRouteLoad(
					waitForAttendanceAccess(_meta.menu.permission).then((allowed) =>
						allowed ? attendanceReport(term, undefined, { requestFetch: fetch }) : null
					),
					'โหลดข้อมูลเช็คชื่อไม่ได้'
				)
			: null
	};
};
