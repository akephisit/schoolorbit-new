import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readCare, careRoutineReadPermissions } from '#lib/features/student-care/access.js';

import { careVisit } from '#lib/api/student-care.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: careRoutineReadPermissions }
};
export const load: PageLoad = ({ fetch, params, depends }) => {
	depends('school:app-identity');
	return {
		id: params.id,
		initial: captureRouteLoad(
			readCare(careRoutineReadPermissions, () => careVisit(params.id, { requestFetch: fetch })),
			'โหลดบันทึกเยี่ยมบ้านไม่ได้'
		)
	};
};
