import type { PageLoad } from './$types';
import { getCurriculumOverview } from '#lib/api/academic-core.js';
import { CURRICULUM_OVERVIEW_DEPENDENCY } from '#lib/academic-core/foundation-route.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';

export const _meta = {
	academicContext: 'none',
	menu: {
		title: 'หลักสูตรและแผนการเรียน',
		icon: 'BookCopy',
		group: 'academic_curriculum',
		workspace: 'academic',
		order: 30,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_CURRICULUM
	}
};

export const load: PageLoad = ({ depends, fetch }) => {
	depends(CURRICULUM_OVERVIEW_DEPENDENCY);
	return {
		title: _meta.menu.title,
		overview: captureRouteLoad(
			getCurriculumOverview({ requestFetch: fetch }),
			'โหลดภาพรวมหลักสูตรไม่สำเร็จ'
		)
	};
};
