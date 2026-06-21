import { PERMISSIONS } from '$lib/permissions/registry';
import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.SUPERVISION_EVALUATE_ASSIGNED
	}
};

export const load: PageLoad = async () => {
	return {
		title: 'รายการที่ต้องประเมิน'
	};
};
