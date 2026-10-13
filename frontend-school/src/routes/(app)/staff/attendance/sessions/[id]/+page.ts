import type { PageLoad } from './$types';
import { attendanceDetail, attendanceWorkspace } from '#lib/api/attendance.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import {
	readAttendance,
	ATTENDANCE_STAFF_PERMISSIONS
} from '#lib/features/attendance/attendance-access.js';
export const _meta = {
	academicContext: 'term_required' as const,
	access: { user_type: 'staff', permission: ATTENDANCE_STAFF_PERMISSIONS }
};
export const load: PageLoad = ({ fetch, params, depends }) => {
	depends('school:app-identity');
	return {
		initial: captureRouteLoad(
			readAttendance(ATTENDANCE_STAFF_PERMISSIONS, async () => {
				const detail = await attendanceDetail(params.id, { requestFetch: fetch });
				const workspace = await attendanceWorkspace(
					detail.session.academicTermId,
					detail.session.date,
					{ requestFetch: fetch }
				);
				return { detail, workspace };
			}),
			'โหลดรอบเช็กชื่อไม่ได้'
		),
		id: params.id
	};
};
