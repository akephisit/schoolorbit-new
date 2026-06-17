import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.STAFF_UPDATE_ALL
	}
};

export const load = async () => {
	return {
		title: 'แก้ไขข้อมูลบุคลากร'
	};
};
