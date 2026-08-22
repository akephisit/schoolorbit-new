import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'คลังฟอนต์โรงเรียน',
		icon: 'Type',
		group: 'settings',
		workspace: 'settings',
		order: 920,
		user_type: 'staff',
		permission: PERMISSIONS.FONT_MANAGE_SCHOOL
	}
};

export const load = async () => ({
	title: 'คลังฟอนต์โรงเรียน'
});
