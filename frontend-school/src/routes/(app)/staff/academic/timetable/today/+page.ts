import type { PageLoad } from './$types';
import { currentLocalDate, getDailyTeachingOverview } from '#lib/api/timetable.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';

export const _meta = {
	academicContext: 'term_required' as const,
	access: {
		user_type: 'staff',
		permission: [
			PERMISSIONS.ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_TIMETABLE_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_TIMETABLE_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_TIMETABLE_PUBLISH_SCHOOL
		]
	},
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
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const academicTermId = url.searchParams.get('academicTermId')?.trim() || null;
	const initialDate = currentLocalDate();
	return {
		title: _meta.menu.title,
		academicYearId,
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
