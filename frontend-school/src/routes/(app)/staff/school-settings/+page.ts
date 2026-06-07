import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'ตั้งค่าโรงเรียน',
		icon: 'School',
		group: 'settings',
		order: 900,
		user_type: 'staff',
		permission: PERMISSIONS.SETTINGS_UPDATE_ALL
	}
};

export const load = async () => {
	return {
		title: 'ตั้งค่าโรงเรียน'
	};
};
