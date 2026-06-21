import { PERMISSIONS } from '$lib/permissions/registry';
import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: [
			PERMISSIONS.SUPERVISION_MANAGE_SCHOOL,
			PERMISSIONS.SUPERVISION_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.SUPERVISION_MANAGE_ORGANIZATION_TREE
		]
	}
};

export const load: PageLoad = async () => {
	return {
		title: 'คำขอจองนิเทศ'
	};
};
