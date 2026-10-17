import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readCare, careReadPermissions } from '#lib/features/student-care/access.js';

import { careInstrument } from '#lib/api/student-care.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: careReadPermissions }
};
export const load: PageLoad = ({ fetch, params, url, depends }) => {
	depends('school:app-identity');
	return {
		id: params.id,
		term: url.searchParams.get('academicTermId'),
		initial: captureRouteLoad(
			readCare(careReadPermissions, () => careInstrument(params.id, { requestFetch: fetch })),
			'โหลดแบบประเมินไม่ได้'
		)
	};
};
