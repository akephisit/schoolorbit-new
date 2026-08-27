import { PERMISSION_MODULES } from '$lib/permissions/registry';
import type { PageLoad } from './$types';

export const _meta = {
	academicContext: 'term_optional' as const,
	menu: {
		title: 'นิเทศการสอน',
		icon: 'ClipboardCheck',
		group: 'academic_supervision',
		workspace: 'academic',
		order: 10,
		user_type: 'staff',
		permission: PERMISSION_MODULES.SUPERVISION
	}
};

export const load: PageLoad = async () => {
	return {
		title: _meta.menu.title
	};
};
