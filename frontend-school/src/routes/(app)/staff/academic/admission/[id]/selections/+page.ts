/**
 * Admission Selections & Room Assignment Page
 */

import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.ADMISSION_SCORES_ALL
	}
};

export const load = async () => {
	return {
		title: 'จัดห้องเรียน'
	};
};
