import type { PageLoad } from './$types';
import { currentLocalDate, getDailyTeachingOverview } from '$lib/api/timetable';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'ตารางสอนวันนี้',
		icon: 'CalendarClock',
		group: 'academic_delivery',
		workspace: 'academic',
		permission: PERMISSION_MODULES.ACADEMIC_TIMETABLE_TODAY,
		order: 30,
		user_type: 'staff'
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const academicTermId = url.searchParams.get('academicTermId')?.trim() || null;
	const initialDate = currentLocalDate();
	return {
		title: _meta.menu.title,
		academicTermId,
		initialDate,
		overview: academicTermId
			? captureRouteLoad(
					getDailyTeachingOverview(
						{ academicTermId, date: initialDate, includeEmptyTeachers: false },
						{ requestFetch: fetch }
					),
					'โหลดตารางสอนรายวันไม่สำเร็จ'
				)
			: null
	};
};
