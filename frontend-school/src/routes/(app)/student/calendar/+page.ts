export const _meta = {
	menu: {
		title: 'ปฏิทิน',
		icon: 'CalendarDays',
		group: 'main',
		workspace: 'home',
		order: 3,
		user_type: 'student'
	}
};
import type { PageLoad } from './$types';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { listMyCalendarEvents } from '#lib/api/calendar.js';
import { calendarRouteFilters } from '#lib/utils/calendar-route-filters.js';
import { calendarGridRange } from '#lib/utils/calendar.js';
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const month = calendarRouteFilters(url).month,
		requestKey = `${url.pathname}|${month}`;
	const records = captureRouteLoad(
		waitForAuthenticatedUser().then(async (user) => ({
			ownerKey: `${appIdentityKey()}|${requestKey}`,
			records:
				user?.user_type === 'student'
					? await listMyCalendarEvents(calendarGridRange(month), { requestFetch: fetch })
					: []
		})),
		'โหลดปฏิทินไม่สำเร็จ'
	);
	return { title: 'ปฏิทิน', requestHref: url.href, requestKey, month, records, studentId: null };
};
