import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: [
			PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_READ_SCHOOL
		]
	}
};

export const load = async () => ({
	title: 'ออกแบบเกียรติบัตร'
});
