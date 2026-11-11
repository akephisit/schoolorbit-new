import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import type { PageLoad } from './$types';
import { getAcademicSetupWorkspace } from '#lib/api/academic-core.js';
import { ACADEMIC_SETUP_WORKSPACE_DEPENDENCY } from '#lib/academic-core/foundation-route.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';

export const _meta = {
	academicContext: 'none',
	menu: {
		title: 'เวลาเรียนและคาบเรียน',
		icon: 'CalendarRange',
		group: 'academic_delivery',
		workspace: 'academic',
		order: 10,
		user_type: 'staff',
		permission: [PERMISSION_MODULES.ACADEMIC_YEAR, PERMISSION_MODULES.ACADEMIC_TERM]
	}
};

export const load: PageLoad = ({ depends, fetch }) => {
	depends('school:app-identity', ACADEMIC_SETUP_WORKSPACE_DEPENDENCY);
	return {
		title: _meta.menu.title,
		workspace: captureRouteLoad(
			waitForAuthenticatedUser().then((user) =>
				user?.user_type === 'staff' &&
				get(can).hasAny(
					PERMISSIONS.ACADEMIC_YEAR_READ_SCHOOL,
					PERMISSIONS.ACADEMIC_YEAR_MANAGE_SCHOOL,
					PERMISSIONS.ACADEMIC_TERM_READ_SCHOOL,
					PERMISSIONS.ACADEMIC_TERM_MANAGE_SCHOOL
				)
					? getAcademicSetupWorkspace({ requestFetch: fetch })
					: null
			),
			'โหลดโครงสร้างปีการศึกษาไม่สำเร็จ'
		)
	};
};
