import { PERMISSIONS } from '$lib/permissions/registry';
import type { PageLoad } from './$types';
import { getPromotionRun, getPromotionPolicyOptions } from '$lib/api/academic-promotion';
import { waitForPromotionReadAccess } from '$lib/academic/lifecycle/promotion-access';
import { captureRouteLoad } from '$lib/navigation/route-load';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL }
};
export const load: PageLoad = ({ params, fetch }) => {
	const access = waitForPromotionReadAccess();
	return {
		runId: params.id,
		title: 'ตรวจรอบเลื่อนชั้น',
		workspace: captureRouteLoad(
			access.then((allowed) =>
				allowed ? getPromotionRun(params.id, { requestFetch: fetch }) : null
			),
			'โหลดรายละเอียดรอบไม่สำเร็จ'
		),
		options: captureRouteLoad(
			access.then((allowed) =>
				allowed ? getPromotionPolicyOptions({ requestFetch: fetch }) : null
			),
			'โหลดชื่อชั้นและแผนไม่สำเร็จ'
		)
	};
};
