import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'คำขอออกเกียรติบัตร',
		icon: 'ClipboardCheck',
		group: 'academic',
		workspace: 'academic',
		order: 61,
		user_type: 'staff',
		permission: PERMISSIONS.CERTIFICATE_ISSUE_SCHOOL
	},
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.CERTIFICATE_ISSUE_SCHOOL
	}
};

export const load = async () => ({
	title: _meta.menu.title
});
