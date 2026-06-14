import { PERMISSION_MODULES } from '$lib/permissions/registry';
import type { PageLoad } from './$types';

export const _meta = {
	menu: {
		title: 'นิเทศการสอน',
		icon: 'ClipboardCheck',
		group: 'academic',
		workspace: 'academic',
		order: 11,
		user_type: 'staff',
		permission: PERMISSION_MODULES.SUPERVISION
	}
};

export const load: PageLoad = async () => {
	return {
		title: _meta.menu.title
	};
};
