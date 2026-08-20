import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const ssr = false;

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACHIEVEMENT
	}
};

export const load = async () => ({
	title: 'ผลงานที่บันทึกเอง'
});
