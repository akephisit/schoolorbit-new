/**
 * Create New Admission Round Page
 */

import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.ADMISSION_MANAGE_ALL
	}
};

export const load = async () => {
	return {
		title: 'สร้างรอบรับสมัครใหม่'
	};
};
