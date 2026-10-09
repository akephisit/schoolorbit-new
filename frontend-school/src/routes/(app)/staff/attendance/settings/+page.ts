import type { PageLoad } from './$types';
import {
	currentAttendanceDate,
	attendanceSettings,
	attendanceOptions,
	attendanceDays
} from '#lib/api/attendance.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'ตั้งค่าเช็คชื่อ',
		icon: 'CalendarDays',
		group: 'academic_delivery',
		workspace: 'academic',
		permission: PERMISSION_MODULES.ATTENDANCE,
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
	return {
		title: _meta.menu.title,
		term,
		date,
		initial: term
			? captureRouteLoad(
					Promise.all([
						attendanceSettings(term, { requestFetch: fetch }),
						attendanceOptions(term, { requestFetch: fetch }),
						attendanceDays(term, monthStart, monthEnd, { requestFetch: fetch })
					]),
					'โหลดข้อมูลเช็คชื่อไม่ได้'
				)
			: null
	};
};
