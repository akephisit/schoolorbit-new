import { listSupervisionCalendar } from '#lib/api/supervision.js';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { schoolCivilDate } from '#lib/supervision/booking-range.js';
import { calendarGridRange } from '#lib/utils/calendar.js';
import type { PageLoad } from './$types';

export const _meta = {
	academicContext: 'term_optional' as const,
	academicContextOptions: 'published' as const,
	access: {
		user_type: 'staff'
	}
};

export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity', 'school:supervision-calendar');
	const academicYearId = url.searchParams.get('academicYearId');
	const academicTermId = url.searchParams.get('academicTermId');
	const requestedMonth = url.searchParams.get('month');
	const today = schoolCivilDate();
	const month =
		requestedMonth && /^\d{4}-(0[1-9]|1[0-2])$/.test(requestedMonth)
			? requestedMonth
			: today.slice(0, 7);
	const monthDate = `${month}-01`;
	const { from, to } = calendarGridRange(monthDate);
	const schedule = academicYearId
		? waitForAuthenticatedUser().then(async (user) => {
				if (user?.user_type !== _meta.access.user_type) return null;
				const identity = appIdentityKey();
				return {
					identity,
					result: await captureRouteLoad(
						listSupervisionCalendar(
							{
								academicYearId,
								...(academicTermId ? { academicTermId } : {}),
								fromDate: from,
								toDate: to
							},
							{ requestFetch: fetch }
						),
						'โหลดปฏิทินนิเทศไม่สำเร็จ'
					)
				};
			})
		: null;

	return {
		title: 'ปฏิทินนิเทศ',
		today,
		month,
		monthDate,
		schedule
	};
};
