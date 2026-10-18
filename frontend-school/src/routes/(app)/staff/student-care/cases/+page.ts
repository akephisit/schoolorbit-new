import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readCare, careSensitiveReadPermissions } from '#lib/features/student-care/access.js';

import { careCases } from '#lib/api/student-care.js';
import { careListQuery } from '#lib/features/student-care/route.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: careSensitiveReadPermissions }
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const query = careListQuery(url);
	return {
		term: url.searchParams.get('academicTermId'),
		url: url.href,
		query,
		initial: captureRouteLoad(
			readCare(careSensitiveReadPermissions, () => careCases(query, { requestFetch: fetch })),
			'โหลดการดูแลรายกรณีไม่ได้'
		)
	};
};
