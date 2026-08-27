/**
 * Admission Management — รายการรอบรับสมัครทั้งหมด
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'year_required' as const,
	menu: {
		title: 'รับสมัครนักเรียน',
		icon: 'ClipboardList',
		group: 'academic_admission',
		workspace: 'academic',
		order: 10,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ADMISSION
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
