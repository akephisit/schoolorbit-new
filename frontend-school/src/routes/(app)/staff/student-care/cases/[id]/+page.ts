import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readCare, careSensitiveReadPermissions } from '#lib/features/student-care/access.js';

import { careCase } from '#lib/api/student-care.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: careSensitiveReadPermissions }
};
export const load: PageLoad = ({ fetch, params, url, depends }) => {
	depends('school:app-identity');
	return {
		id: params.id,
		term: url.searchParams.get('academicTermId'),
		initial: captureRouteLoad(
			readCare(careSensitiveReadPermissions, () => careCase(params.id, { requestFetch: fetch })),
			'โหลดการดูแลไม่ได้'
		)
	};
};
