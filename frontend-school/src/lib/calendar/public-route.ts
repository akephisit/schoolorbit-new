import { publicRequestFromOrigin } from '#lib/api/client.js';
import { listPublicCalendarEvents } from '#lib/api/calendar.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { calendarRouteFilters } from '#lib/utils/calendar-route-filters.js';
import { publicCalendarRange } from '#lib/utils/calendar.js';

export function loadPublicCalendar(requestFetch: typeof fetch, url: URL) {
	const options = publicRequestFromOrigin(requestFetch, url.origin);
	const month = calendarRouteFilters(url).month;
	const events = captureRouteLoad(
		listPublicCalendarEvents(publicCalendarRange(month), options),
		'โหลดปฏิทินไม่สำเร็จ'
	);
	return { title: 'ปฏิทินโรงเรียน', month, events };
}
export type PublicCalendarRouteData = ReturnType<typeof loadPublicCalendar>;
