import type { PageLoad } from './$types';
import { getTimetable, getTimetableBlockWorkspace } from '#lib/api/timetable.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'ตารางสอน',
		icon: 'CalendarDays',
		group: 'academic_delivery',
		workspace: 'academic',
		permission: PERMISSION_MODULES.ACADEMIC_TIMETABLE,
		order: 40,
		user_type: 'staff'
	}
};
export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const academicTermId = url.searchParams.get('academicTermId')?.trim() || null;
	const workspace =
		academicYearId && academicTermId
			? captureRouteLoad(
					getTimetable(academicTermId, { requestFetch: fetch }).then((timetable) =>
						timetable
							? getTimetableBlockWorkspace(
									{ academicYearId, academicTermId, timetableId: timetable.id },
									{ requestFetch: fetch }
								)
							: null
					),
					'โหลดตารางสอนไม่สำเร็จ'
				)
			: null;
	return { title: _meta.menu.title, academicYearId, academicTermId, workspace };
};
