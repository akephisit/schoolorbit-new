import { PERMISSIONS } from '$lib/permissions/registry';

export const ssr = false;

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.CERTIFICATE_READ_OWN
	}
};

export const load = async () => ({
	title: 'เกียรติบัตรที่โรงเรียนออก'
});
