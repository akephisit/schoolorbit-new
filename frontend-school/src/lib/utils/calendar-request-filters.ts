import type { CalendarRequestQuery } from '#lib/api/calendar.js';
export function calendarRequestFilters(url: URL): CalendarRequestQuery {
	const value = url.searchParams.get('status'),
		rawOffset = Number(url.searchParams.get('offset') ?? 0),
		review = url.searchParams.get('review') === 'true';
	return {
		review,
		status: review
			? value === 'rejected'
				? 'rejected'
				: value === 'all'
					? undefined
					: 'pending'
			: value === 'pending' || value === 'approved' || value === 'rejected'
				? value
				: undefined,
		offset: Number.isInteger(rawOffset) && rawOffset >= 0 && rawOffset <= 100000 ? rawOffset : 0
	};
}
