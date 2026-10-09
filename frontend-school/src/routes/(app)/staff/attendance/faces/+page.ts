import type { PageLoad } from './$types';
import { currentAttendanceDate, attendanceOptions } from '#lib/api/attendance.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'เว็บแคมและใบหน้า',
		icon: 'ScanFace',
		group: 'academic_delivery',
		workspace: 'academic',
		permission: PERMISSION_MODULES.ATTENDANCE,
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
					attendanceOptions(term, { requestFetch: fetch }),
					'โหลดข้อมูลเช็คชื่อไม่ได้'
				)
			: null
	};
};
