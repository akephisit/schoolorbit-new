import type { CalendarRequestQuery } from '#lib/api/calendar.js';
export function calendarRequestFilters(url: URL): CalendarRequestQuery {
	const value = url.searchParams.get('status'),
		rawOffset = Number(url.searchParams.get('offset') ?? 0);
	return {
		review: url.searchParams.get('review') === 'true',
		status: value === 'pending' || value === 'approved' || value === 'rejected' ? value : undefined,
		offset: Number.isInteger(rawOffset) && rawOffset >= 0 && rawOffset <= 100000 ? rawOffset : 0
	};
}
