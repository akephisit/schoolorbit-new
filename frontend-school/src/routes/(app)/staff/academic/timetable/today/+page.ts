import { get } from 'svelte/store';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { can } from '#lib/stores/permissions.js';
import type { PageLoad } from './$types';
import { currentLocalDate, getDailyTeachingOverview } from '#lib/api/timetable.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';

export const _meta = {
	academicContext: 'term_required' as const,
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL
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

export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
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
					waitForAuthenticatedUser().then((user) =>
						user?.user_type === 'staff' &&
						get(can).has(PERMISSIONS.ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL)
							? getDailyTeachingOverview(
									{ academicTermId, date: initialDate, includeEmptyTeachers: false },
									{ requestFetch: fetch }
								)
							: null
					),
					'โหลดตารางสอนรายวันไม่สำเร็จ'
				)
			: null
	};
};
