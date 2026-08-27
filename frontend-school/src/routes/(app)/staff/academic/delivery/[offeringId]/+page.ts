import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'required' as const,
	access: {
		user_type: 'staff',
		permission: PERMISSION_MODULES.LEARNING_OFFERING
	}
};

export const load = () => ({ title: 'รายละเอียดรายการเปิดสอน' });
