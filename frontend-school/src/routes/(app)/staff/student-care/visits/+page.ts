import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readCare, careRoutineReadPermissions } from '#lib/features/student-care/access.js';

import { careVisits } from '#lib/api/student-care.js';
import { careListQuery } from '#lib/features/student-care/route.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: careRoutineReadPermissions }
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const query = careListQuery(url);
	return {
		term: url.searchParams.get('academicTermId'),
		url: url.href,
		query,
		initial: captureRouteLoad(
			readCare(careRoutineReadPermissions, () => careVisits(query, { requestFetch: fetch })),
			'โหลดเยี่ยมบ้านไม่ได้'
		)
	};
};
