/**
 * Course Planning Page
 */

import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'จัดแผนการเรียน',
		icon: 'BookOpen',
		group: 'academic',
		order: 30, // ถัดจาก ห้องเรียน (20)
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_COURSE_PLAN_READ_ALL
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
