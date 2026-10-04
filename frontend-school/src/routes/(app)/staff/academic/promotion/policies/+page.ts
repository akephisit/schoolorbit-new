import type { PageLoad } from './$types';
import { waitForPromotionReadAccess } from '#lib/academic/lifecycle/promotion-access.js';
import { listPromotionPolicies } from '#lib/api/academic-promotion.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';

export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'เกณฑ์การเลื่อนชั้น',
		icon: 'ListChecks',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 80,
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL
	}
};
export const load: PageLoad = ({ fetch }) => ({
	title: _meta.menu.title,
	policies: captureRouteLoad(
		waitForPromotionReadAccess().then((allowed) =>
			allowed ? listPromotionPolicies({ requestFetch: fetch }) : []
		),
		'โหลดเกณฑ์เลื่อนชั้นไม่สำเร็จ'
	)
});
