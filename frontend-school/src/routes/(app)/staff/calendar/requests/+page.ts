import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { listCalendarRequests } from '#lib/api/calendar.js';
import { calendarRequestFilters } from '#lib/utils/calendar-request-filters.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.CALENDAR_READ_SCHOOL }
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const query = calendarRequestFilters(url);
	const requestKey = JSON.stringify(query);
	return {
		title: 'คำร้องเพิ่มกิจกรรม',
		query,
		requestKey,
		requests: captureRouteLoad(
			waitForAuthenticatedUser().then(async (user) => {
				const identity = appIdentityKey();
				const authorized =
					user?.user_type === 'staff' &&
					get(can).has(PERMISSIONS.CALENDAR_READ_SCHOOL) &&
					(query.review
						? get(can).has(PERMISSIONS.CALENDAR_MANAGE_SCHOOL)
						: get(can).hasAny(
								PERMISSIONS.CALENDAR_REQUEST_OWN,
								PERMISSIONS.CALENDAR_MANAGE_SCHOOL
							));
				return {
					ownerKey: `${identity}|${requestKey}`,
					page: authorized ? await listCalendarRequests(query, { requestFetch: fetch }) : null
				};
			}),
			'โหลดคำร้องไม่สำเร็จ'
		)
	};
};
