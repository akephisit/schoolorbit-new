import { PERMISSION_MODULES, PERMISSIONS } from '$lib/permissions/registry';

const achievementsAccess = [PERMISSION_MODULES.ACHIEVEMENT, PERMISSIONS.CERTIFICATE_READ_OWN];

export const ssr = false;

export const _meta = {
	menu: {
		title: 'เกียรติบัตรและผลงาน',
		icon: 'Award',
		group: 'personnel',
		workspace: 'personnel',
		order: 30,
		user_type: 'staff',
		permission: [PERMISSION_MODULES.ACHIEVEMENT, PERMISSIONS.CERTIFICATE_READ_OWN]
	},
	access: {
		user_type: 'staff',
		permission: achievementsAccess
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
