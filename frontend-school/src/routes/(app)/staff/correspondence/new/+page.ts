import { PERMISSIONS } from '#lib/permissions/registry.js';
export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'สร้างหนังสือ',
		icon: 'FilePlus2',
		group: 'correspondence',
		workspace: 'operations',
		order: 110,
		user_type: 'staff',
		permission: PERMISSIONS.CORRESPONDENCE_CREATE_OWN
	},
	access: { user_type: 'staff', permission: PERMISSIONS.CORRESPONDENCE_CREATE_OWN }
};
