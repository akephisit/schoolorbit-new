import type { PageLoad } from './$types';
import { currentAttendanceDate, attendanceOptions } from '#lib/api/attendance.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { waitForAttendanceAccess } from '#lib/features/attendance/attendance-access.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'เว็บแคมและใบหน้า',
		icon: 'ScanFace',
		group: 'academic_delivery',
		workspace: 'academic',
		permission: [
			PERMISSIONS.ATTENDANCE_ENROLL_ASSIGNED,
			PERMISSIONS.ATTENDANCE_ENROLL_SCHOOL,
			PERMISSIONS.ATTENDANCE_VERIFY_ASSIGNED
		],
		order: 42,
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
						allowed ? attendanceOptions(term, { requestFetch: fetch }) : null
					),
					'โหลดข้อมูลเช็คชื่อไม่ได้'
				)
			: null
	};
};
