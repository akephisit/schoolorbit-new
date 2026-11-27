import { PERMISSION_MODULES } from '#lib/permissions/registry.js';

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
