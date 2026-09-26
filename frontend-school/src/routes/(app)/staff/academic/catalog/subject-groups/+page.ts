import type { PageLoad } from './$types';
import { listSubjectGroups } from '$lib/api/academic-core';
import { CATALOG_SUBJECT_GROUPS_DEPENDENCY } from '$lib/academic-core/catalog-route';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'none',
	menu: {
		title: 'กลุ่มสาระการเรียนรู้',
		icon: 'Layers3',
		group: 'academic_curriculum',
		workspace: 'academic',
		order: 10,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_CATALOG
	}
};

export const load: PageLoad = ({ depends, fetch }) => {
	depends(CATALOG_SUBJECT_GROUPS_DEPENDENCY);
	return {
		title: _meta.menu.title,
		groups: captureRouteLoad(listSubjectGroups({ requestFetch: fetch }), 'โหลดกลุ่มสาระไม่สำเร็จ')
	};
};
