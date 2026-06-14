/**
 * Study Plans Management Page
 * จัดการหลักสูตรสถานศึกษา (ฉบับปรับปรุง พ.ศ. ...)
 */

import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'หลักสูตรสถานศึกษา',
		icon: 'GraduationCap',
		group: 'academic',
		workspace: 'academic',
		order: 3,
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_CURRICULUM_READ_ALL
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
