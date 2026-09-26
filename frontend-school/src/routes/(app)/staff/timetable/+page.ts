import type { PageLoad } from './$types';
import { currentLocalDate, getMyTimetable } from '$lib/api/timetable';
import { captureRouteLoad } from '$lib/navigation/route-load';

export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'ตารางสอน',
		icon: 'CalendarDays',
		group: 'main',
		workspace: 'home',
		order: 2,
		user_type: 'staff'
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const academicTermId = url.searchParams.get('academicTermId')?.trim() || null;
	const date = currentLocalDate();
	return {
		title: _meta.menu.title,
		academicYearId,
		academicTermId,
		blocks: academicTermId
			? captureRouteLoad(
					getMyTimetable({ academicTermId, date }, { requestFetch: fetch }),
					'โหลดตารางสอนของฉันไม่สำเร็จ'
				)
			: null
	};
};
