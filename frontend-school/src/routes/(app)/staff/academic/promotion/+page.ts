import type { PageLoad } from './$types';
import { waitForPromotionReadAccess } from '#lib/academic/lifecycle/promotion-access.js';
import { listPromotionRuns } from '#lib/api/academic-promotion.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
export const _meta = {
	academicContext: 'year_required' as const,
	menu: {
		title: 'เลื่อนชั้นและเตรียมปีใหม่',
		icon: 'GraduationCap',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 85,
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL
	}
};
export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const runs = academicYearId
		? captureRouteLoad(
				waitForPromotionReadAccess().then((allowed) =>
					allowed
						? listPromotionRuns({ sourceYearId: academicYearId }, { requestFetch: fetch })
						: { runs: [], nextCursor: null }
				),
				'โหลดรอบเลื่อนชั้นไม่สำเร็จ'
			)
		: null;
	return { title: _meta.menu.title, academicYearId, runs };
};
