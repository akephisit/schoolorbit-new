import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'ตั้งค่าโรงเรียน',
		icon: 'School',
		group: 'settings',
		workspace: 'settings',
		order: 900,
		user_type: 'staff',
		permission: PERMISSION_MODULES.SETTINGS
	}
};

export const load = async () => {
	return {
		title: 'ตั้งค่าโรงเรียน'
	};
};
