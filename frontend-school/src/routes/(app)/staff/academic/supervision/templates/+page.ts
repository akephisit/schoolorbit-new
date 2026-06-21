import { PERMISSIONS } from '$lib/permissions/registry';
import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.SUPERVISION_MANAGE_SCHOOL
	}
};

export const load: PageLoad = async () => {
	return {
		title: 'แบบประเมินนิเทศ'
	};
};
