import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.STAFF_CREATE_ALL
	}
};

export const load = async () => {
	return {
		title: 'เพิ่มบุคลากรใหม่'
	};
};
