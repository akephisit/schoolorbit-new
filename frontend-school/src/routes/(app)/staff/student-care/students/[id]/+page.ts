import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import {
	readCare,
	careReadPermissions,
	careSensitiveReadPermissions
} from '#lib/features/student-care/access.js';

import { careAssignments, careVisits, careCases } from '#lib/api/student-care.js';
import { careRoutineReadPermissions } from '#lib/features/student-care/access.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: careReadPermissions }
};
export const load: PageLoad = ({ fetch, params, url, depends }) => {
	depends('school:app-identity');
	const query = { studentId: params.id, limit: 50 };
	return {
		id: params.id,
		term: url.searchParams.get('academicTermId'),
		query,
		assignments: captureRouteLoad(
			readCare(careReadPermissions, () => careAssignments(query, { requestFetch: fetch })),
			'โหลดแบบประเมินไม่ได้'
		),
		visits: captureRouteLoad(
			readCare(careRoutineReadPermissions, () => careVisits(query, { requestFetch: fetch })),
			'โหลดเยี่ยมบ้านไม่ได้'
		),
		cases: captureRouteLoad(
			readCare(careSensitiveReadPermissions, () => careCases(query, { requestFetch: fetch })),
			'โหลดการดูแลไม่ได้'
		)
	};
};
