import type { PageLoad } from './$types';
import { getCatalogActivityOverview } from '#lib/api/academic-core.js';
import { CATALOG_ACTIVITY_OVERVIEW_DEPENDENCY } from '#lib/academic-core/catalog-route.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';

export const _meta = {
	academicContext: 'none',
	menu: {
		title: 'ทะเบียนกิจกรรมพัฒนาผู้เรียน',
		icon: 'Sparkles',
		group: 'academic_activities',
		workspace: 'academic',
		order: 10,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_CATALOG
	}
};

export const load: PageLoad = ({ depends, fetch }) => {
	depends(CATALOG_ACTIVITY_OVERVIEW_DEPENDENCY);
	return {
		title: _meta.menu.title,
		overview: captureRouteLoad(
			getCatalogActivityOverview({ requestFetch: fetch }),
			'โหลดทะเบียนกิจกรรมไม่สำเร็จ'
		)
	};
};
