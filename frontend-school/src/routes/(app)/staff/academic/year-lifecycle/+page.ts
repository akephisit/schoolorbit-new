import { get } from 'svelte/store';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { can } from '#lib/stores/permissions.js';
import { getYearLifecycleWorkspace } from '#lib/api/academic-lifecycle.js';
import { getAcademicSetupWorkspace } from '#lib/api/academic-core.js';
import { listPromotionRuns } from '#lib/api/academic-promotion.js';
import { ACADEMIC_SETUP_WORKSPACE_DEPENDENCY } from '#lib/academic-core/foundation-route.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import type { PageLoad } from './$types';

export const _meta = {
	academicContext: 'year_required' as const,
	menu: {
		title: 'จัดการปีการศึกษา',
		icon: 'CalendarRange',
		group: 'academic_delivery',
		workspace: 'academic',
		order: 5,
		user_type: 'staff',
		permission: [
			PERMISSION_MODULES.ACADEMIC_YEAR,
			PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL
		]
	}
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	depends(
		'school:app-identity',
		'schoolorbit:year-management',
		'schoolorbit:rollover-history',
		ACADEMIC_SETUP_WORKSPACE_DEPENDENCY
	);
	const access = waitForAuthenticatedUser();
	return {
		title: _meta.menu.title,
		academicYearId,
		workspace: academicYearId
			? captureRouteLoad(
					access.then((user) =>
						user && get(can).has(PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL)
							? getYearLifecycleWorkspace(academicYearId, { requestFetch: fetch })
							: null
					),
					'โหลดปีการศึกษาไม่สำเร็จ'
				)
			: null,
		setup: captureRouteLoad(
			access.then((user) =>
				user &&
				get(can).hasAny(
					PERMISSIONS.ACADEMIC_YEAR_READ_SCHOOL,
					PERMISSIONS.ACADEMIC_YEAR_MANAGE_SCHOOL
				) &&
				get(can).hasAny(
					PERMISSIONS.ACADEMIC_TERM_READ_SCHOOL,
					PERMISSIONS.ACADEMIC_TERM_MANAGE_SCHOOL
				)
					? getAcademicSetupWorkspace({ requestFetch: fetch })
					: null
			),
			'โหลดตั้งค่าปีและภาคเรียนไม่สำเร็จ'
		),
		history: academicYearId
			? captureRouteLoad(
					access.then((user) =>
						user && get(can).has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL)
							? listPromotionRuns({ sourceYearId: academicYearId }, { requestFetch: fetch })
							: null
					),
					'โหลดประวัติขึ้นปีใหม่ไม่สำเร็จ'
				)
			: null
	};
};
