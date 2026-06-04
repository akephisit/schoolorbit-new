import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'อาคารสถานที่',
		icon: 'School', // Changed to School icon which is more meaningful than Building (generic)
		group: 'general_admin',
		permission: PERMISSIONS.FACILITY_READ_ALL,
		order: 10,
		user_type: 'staff'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
