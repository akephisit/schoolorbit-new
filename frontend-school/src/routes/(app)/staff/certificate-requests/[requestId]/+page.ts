import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.CERTIFICATE_ISSUE_SCHOOL
	}
};

export const load = async () => ({
	title: 'ตรวจคำขอออกเกียรติบัตร'
});
