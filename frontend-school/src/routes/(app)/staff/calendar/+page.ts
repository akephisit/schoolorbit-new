import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '$lib/stores/permissions';
import { waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { calendarRouteFilters } from '$lib/utils/calendar-route-filters';
import { listCalendarEvents, listCalendarCategories, listCalendarTags } from '$lib/api/calendar';
import { PERMISSIONS, PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_optional' as const,
	menu: {
		title: 'ปฏิทินโรงเรียน',
		icon: 'CalendarDays',
		group: 'main',
		workspace: 'home',
		order: 7,
		user_type: 'staff',
		permission: PERMISSION_MODULES.CALENDAR
	}
};

export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const context = calendarRouteFilters(url);
	const allowed = waitForAuthenticatedUser().then(
		(user) => user?.user_type === 'staff' && get(can).has(PERMISSIONS.CALENDAR_READ_SCHOOL)
	);
	const events = context.academicYearId
		? captureRouteLoad(
				allowed.then((read) =>
					read ? listCalendarEvents(context.filters, { requestFetch: fetch }) : []
				),
				'โหลดกิจกรรมไม่สำเร็จ'
			)
		: null;
	const categories = captureRouteLoad(
		allowed.then((read) => (read ? listCalendarCategories({ requestFetch: fetch }) : [])),
		'โหลดหมวดหมู่ไม่สำเร็จ'
	);
	const tags = captureRouteLoad(
		allowed.then((read) => (read ? listCalendarTags({ requestFetch: fetch }) : [])),
		'โหลดแท็กไม่สำเร็จ'
	);
	return { title: _meta.menu.title, eventKey: context.key, events, categories, tags };
};
