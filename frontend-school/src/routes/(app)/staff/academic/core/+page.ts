import type { PageLoad } from './$types';
import { getAcademicSetupWorkspace } from '$lib/api/academic-core';
import { ACADEMIC_SETUP_WORKSPACE_DEPENDENCY } from '$lib/academic-core/foundation-route';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'none',
	menu: {
		title: 'ปีการศึกษา ภาคเรียน และเวลาเรียน',
		icon: 'CalendarRange',
		group: 'academic_delivery',
		workspace: 'academic',
		order: 10,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_YEAR
	}
};

export const load: PageLoad = ({ depends, fetch }) => {
	depends(ACADEMIC_SETUP_WORKSPACE_DEPENDENCY);
	return {
		title: _meta.menu.title,
		workspace: captureRouteLoad(
			getAcademicSetupWorkspace({ requestFetch: fetch }),
			'โหลดโครงสร้างปีการศึกษาไม่สำเร็จ'
		)
	};
};
