/**
 * Study Plans Management Page
 * จัดการหลักสูตรสถานศึกษา (ฉบับปรับปรุง พ.ศ. ...)
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'หลักสูตรสถานศึกษา',
		icon: 'GraduationCap',
		group: 'academic',
		workspace: 'academic',
		order: 3,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_CURRICULUM
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
