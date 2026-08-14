import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: [
			PERMISSIONS.CERTIFICATE_SUBMIT_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_SUBMIT_SCHOOL
		]
	}
};

export const load = async () => ({
	title: 'ประวัติคำขอออกเกียรติบัตร'
});
