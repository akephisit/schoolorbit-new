import type { CalendarAudienceType, CalendarEventFilters } from '$lib/api/calendar';
import { calendarGridRange } from './calendar';
export function calendarRouteFilters(
	url: URL,
	today = new Intl.DateTimeFormat('en-CA', {
		timeZone: 'Asia/Bangkok',
		year: 'numeric',
		month: '2-digit',
		day: '2-digit'
	}).format(new Date())
) {
	const rawMonth = url.searchParams.get('month');
	const month =
		rawMonth && /^\d{4}-(0[1-9]|1[0-2])$/.test(rawMonth)
			? `${rawMonth}-01`
			: `${today.slice(0, 7)}-01`;
	const rawAudience = url.searchParams.get('audience') ?? '';
	const audience: CalendarAudienceType | '' = ['all', 'staff', 'student', 'parent'].includes(
		rawAudience
	)
		? (rawAudience as CalendarAudienceType)
		: '';
	const rawVisibility = url.searchParams.get('visibility');
	const visibility: '' | 'public' | 'private' =
		rawVisibility === 'public' || rawVisibility === 'private' ? rawVisibility : '';
	const academicYearId = url.searchParams.get('academicYearId') ?? '';
	const academicTermId = url.searchParams.get('academicTermId') ?? null;
	const categoryId = url.searchParams.get('categoryId') ?? '',
		tagId = url.searchParams.get('tagId') ?? '',
		q = (url.searchParams.get('q') ?? '').trim();
	const filters: CalendarEventFilters = {
		academicYearId,
		academicTermId: academicTermId || undefined,
		...calendarGridRange(month),
		categoryId: categoryId || undefined,
		tagId: tagId || undefined,
		audience: audience || undefined,
		visibility: visibility || undefined,
		q: q || undefined
	};
	return {
		academicYearId,
		academicTermId,
		month,
		categoryId,
		tagId,
		audience,
		visibility,
		q,
		filters,
		key: JSON.stringify(filters)
	};
}
