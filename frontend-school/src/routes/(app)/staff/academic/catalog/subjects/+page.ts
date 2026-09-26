import type { PageLoad } from './$types';
import { getCatalogSubjectOverview } from '$lib/api/academic-core';
import { CATALOG_SUBJECT_OVERVIEW_DEPENDENCY } from '$lib/academic-core/catalog-route';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'none',
	menu: {
		title: 'ทะเบียนรายวิชา',
		icon: 'LibraryBig',
		group: 'academic_curriculum',
		workspace: 'academic',
		order: 20,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_CATALOG
	}
};

export const load: PageLoad = ({ depends, fetch }) => {
	depends(CATALOG_SUBJECT_OVERVIEW_DEPENDENCY);
	return {
		title: _meta.menu.title,
		overview: captureRouteLoad(
			getCatalogSubjectOverview({ requestFetch: fetch }),
			'โหลดทะเบียนรายวิชาไม่สำเร็จ'
		)
	};
};
