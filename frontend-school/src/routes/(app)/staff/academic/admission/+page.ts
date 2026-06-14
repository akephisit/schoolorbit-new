/**
 * Admission Management — รายการรอบรับสมัครทั้งหมด
 */

import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'รับสมัครนักเรียน',
		icon: 'ClipboardList',
		group: 'academic',
		workspace: 'academic',
		order: 40,
		user_type: 'staff',
		permission: PERMISSIONS.ADMISSION_READ_ALL
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
