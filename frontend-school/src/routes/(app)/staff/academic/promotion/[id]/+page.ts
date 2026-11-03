import { PERMISSIONS } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';
import { getPromotionRun } from '#lib/api/academic-promotion.js';
import { waitForPromotionReadAccess } from '#lib/academic/lifecycle/promotion-access.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL }
};
export const load: PageLoad = ({ params, fetch, depends }) => {
	depends('school:app-identity');
	const access = waitForPromotionReadAccess();
	return {
		runId: params.id,
		title: 'ประวัติขึ้นปีการศึกษาใหม่',
		workspace: captureRouteLoad(
			access.then((allowed) =>
				allowed ? getPromotionRun(params.id, { requestFetch: fetch }) : null
			),
			'โหลดรายละเอียดรอบไม่สำเร็จ'
		)
	};
};
