export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'ตารางสอบ',
		icon: 'CalendarClock',
		group: 'main',
		workspace: 'home',
		order: 3,
		user_type: 'staff'
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const academicTermId = url.searchParams.get('academicTermId')?.trim() || null;
	return {
		title: _meta.menu.title,
		academicYearId,
		academicTermId,
		rounds:
			academicYearId && academicTermId
				? captureRouteLoad(
						listStaffExamSchedules(academicTermId, { requestFetch: fetch }),
						'โหลดตารางสอบสำหรับครูไม่สำเร็จ'
					)
				: null
	};
};
import type { PageLoad } from './$types';
import { listStaffExamSchedules } from '$lib/api/examSchedule';
import { captureRouteLoad } from '$lib/navigation/route-load';
