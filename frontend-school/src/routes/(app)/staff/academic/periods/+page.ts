import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'ตั้งค่าคาบเวลา',
		icon: 'Clock',
		group: 'academic',
		workspace: 'academic',
		permission: PERMISSION_MODULES.ACADEMIC_TERM,
		order: 50,
		user_type: 'staff'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
