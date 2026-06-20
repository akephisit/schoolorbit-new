import { PERMISSION_MODULES } from '$lib/permissions/registry';
import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSION_MODULES.SUPERVISION
	}
};

export const load: PageLoad = async ({ params }) => {
	return {
		title: 'รายละเอียดรายการนิเทศ',
		observationId: params.id
	};
};
