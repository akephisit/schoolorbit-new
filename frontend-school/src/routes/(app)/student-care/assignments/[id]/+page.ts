import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readCare, careReadPermissions } from '#lib/features/student-care/access.js';
import { careAssignment, careRound } from '#lib/api/student-care.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { permission: careReadPermissions }
};
export const load: PageLoad = ({ fetch, params, depends }) => {
	depends('school:app-identity');
	return {
		id: params.id,
		initial: captureRouteLoad(
			readCare(careReadPermissions, async () => {
				const detail = await careAssignment(params.id, { requestFetch: fetch });
				const round = await careRound(detail.assignment.roundId, { requestFetch: fetch });
				return { detail, round };
			}),
			'โหลดแบบตอบไม่ได้'
		)
	};
};
