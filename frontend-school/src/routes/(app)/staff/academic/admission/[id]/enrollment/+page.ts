/**
 * Admission Enrollment (มอบตัว) Page
 */

import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.ADMISSION_ENROLL_ALL
	}
};

export const load = async () => {
	return {
		title: 'รับมอบตัว'
	};
};
