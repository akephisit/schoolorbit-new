import { publicRequestFromOrigin } from '#lib/api/client.js';
import { listPublicCalendarEvents } from '#lib/api/calendar.js';
import {
	listPublicAcademicContextOptions,
	type AcademicYearOption
} from '#lib/api/academic-context.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { calendarRouteFilters } from '#lib/utils/calendar-route-filters.js';
import { publicCalendarRange } from '#lib/utils/calendar.js';

export function loadPublicCalendar(requestFetch: typeof fetch, url: URL) {
	const options = publicRequestFromOrigin(requestFetch, url.origin);
	const month = calendarRouteFilters(url).month;
	const yearId = url.searchParams.get('academicYearId')?.trim() ?? '';
	const context = listPublicAcademicContextOptions(undefined, options);
	const years = captureRouteLoad(
		context.then((value) => value.years),
		'โหลดปีการศึกษาไม่สำเร็จ'
	);
	const events = captureRouteLoad(
		(async () => {
			let year: AcademicYearOption | undefined;
			if (yearId) {
				year = (await context).years.find((value) => value.id === yearId);
				if (!year) throw new Error('ไม่พบปีการศึกษาที่เลือก กรุณาเลือกปีใหม่');
			}
			const range = publicCalendarRange(month, year);
			return range.from > range.to ? [] : listPublicCalendarEvents(range, options);
		})(),
		'โหลดปฏิทินไม่สำเร็จ'
	);
	return { title: 'ปฏิทินโรงเรียน', month, yearId, events, years };
}
export type PublicCalendarRouteData = ReturnType<typeof loadPublicCalendar>;
