import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { getLeaveUsage } from '#lib/api/staff-leave.js';
import { captureLeaveRead } from '#lib/staff-leave/route-data.js';
import { schoolToday } from '#lib/staff-leave/presentation.js';
import { validReportDate } from '#lib/staff-leave/report.js';
export const _meta = {
	academicContext: 'none' as const,
	access: {
		user_type: 'staff',
		permission: [PERMISSIONS.STAFF_LEAVE_DOWNLOAD_OWN, PERMISSIONS.STAFF_LEAVE_DOWNLOAD_SCHOOL]
	}
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const today = schoolToday();
	const from = validReportDate(url.searchParams.get('from')) ?? `${today.slice(0, 4)}-01-01`;
	const to = validReportDate(url.searchParams.get('to')) ?? today;
	const kind =
		url.searchParams.get('kind') === 'exit'
			? ('exit' as const)
			: url.searchParams.get('kind') === 'leave'
				? ('leave' as const)
				: undefined;
	const query = { from, to, kind };
	return {
		title: 'สรุปการใช้สิทธิ์',
		query,
		requestKey: JSON.stringify(query),
		report: captureLeaveRead(
			() =>
				get(can).hasAny(
					PERMISSIONS.STAFF_LEAVE_DOWNLOAD_OWN,
					PERMISSIONS.STAFF_LEAVE_DOWNLOAD_SCHOOL
				),
			() => getLeaveUsage({ requestFetch: fetch, query }),
			null
		)
	};
};
