import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: [
			PERMISSIONS.CERTIFICATE_CREATE_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_CREATE_SCHOOL
		]
	}
};

export const load = async () => ({
	title: 'สร้างกิจกรรมเกียรติบัตร'
});
