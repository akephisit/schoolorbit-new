import { PERMISSIONS } from '$lib/permissions/registry';

const managementReadPermissions = [
	PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT,
	PERMISSIONS.CERTIFICATE_READ_SCHOOL
];

export const _meta = {
	menu: {
		title: 'เกียรติบัตร',
		icon: 'Award',
		group: 'academic',
		workspace: 'academic',
		order: 60,
		user_type: 'staff',
		permission: [
			PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_READ_SCHOOL
		]
	},
	access: {
		user_type: 'staff',
		permission: managementReadPermissions
	}
};

export const load = async () => ({
	title: _meta.menu.title
});
