import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readCare, careReadPermissions } from '#lib/features/student-care/access.js';

import { careRound, careAssignments } from '#lib/api/student-care.js';
import { careListQuery } from '#lib/features/student-care/route.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: careReadPermissions }
};
export const load: PageLoad = ({ fetch, params, url, depends }) => {
	depends('school:app-identity');
	const query = { ...careListQuery(url), roundId: params.id };
	return {
		id: params.id,
		url: url.href,
		term: url.searchParams.get('academicTermId'),
		query,
		round: captureRouteLoad(
			readCare(careReadPermissions, () => careRound(params.id, { requestFetch: fetch })),
			'โหลดรอบไม่ได้'
		),
		assignments: captureRouteLoad(
			readCare(careReadPermissions, () => careAssignments(query, { requestFetch: fetch })),
			'โหลดการมอบหมายไม่ได้'
		)
	};
};
