/**
 * Admission Management — รายการรอบรับสมัครทั้งหมด
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'รับสมัครนักเรียน',
		icon: 'ClipboardList',
		group: 'academic',
		workspace: 'academic',
		order: 40,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ADMISSION
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
