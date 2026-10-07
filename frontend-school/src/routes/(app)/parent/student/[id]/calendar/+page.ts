export const _meta = { access: { user_type: 'parent' } };
import type { PageLoad } from './$types';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { listChildCalendarEvents } from '#lib/api/calendar.js';
import { calendarRouteFilters } from '#lib/utils/calendar-route-filters.js';
import { calendarGridRange } from '#lib/utils/calendar.js';
export const load: PageLoad = ({ fetch, url, depends, params }) => {
	depends('school:app-identity');
	const month = calendarRouteFilters(url).month,
		requestKey = `${url.pathname}|${month}`;
	const records = captureRouteLoad(
		waitForAuthenticatedUser().then(async (user) => ({
			ownerKey: `${appIdentityKey()}|${requestKey}`,
			records:
				user?.user_type === 'parent'
					? await listChildCalendarEvents(params.id, calendarGridRange(month), {
							requestFetch: fetch
						})
					: []
		})),
		'โหลดปฏิทินไม่สำเร็จ'
	);
	return {
		title: 'ปฏิทินของลูก',
		requestHref: url.href,
		requestKey,
		month,
		records,
		studentId: params.id
	};
};
