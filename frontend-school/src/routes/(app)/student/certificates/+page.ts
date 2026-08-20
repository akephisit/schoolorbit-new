import { PERMISSIONS } from '$lib/permissions/registry';

export const ssr = false;

export const _meta = {
	menu: {
		title: 'เกียรติบัตรของฉัน',
		icon: 'Award',
		group: 'main',
		workspace: 'home',
		order: 6,
		user_type: 'student',
		permission: PERMISSIONS.CERTIFICATE_READ_OWN
	},
	access: {
		user_type: 'student',
		permission: PERMISSIONS.CERTIFICATE_READ_OWN
	}
};

export const load = async () => ({
	title: _meta.menu.title
});
