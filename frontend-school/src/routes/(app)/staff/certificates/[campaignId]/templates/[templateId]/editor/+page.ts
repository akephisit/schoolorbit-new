import { PERMISSIONS } from '#lib/permissions/registry.js';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: [
			PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_READ_SCHOOL
		]
	}
};

import type { PageLoad } from './$types';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { can } from '#lib/stores/permissions.js';
import { get } from 'svelte/store';
import { getCertificateTemplate } from '#lib/api/certificates.js';
export const load: PageLoad = ({ fetch, depends, params }) => {
	depends('school:app-identity');
	const actor = waitForAuthenticatedUser();
	const ownerKey = () => `${appIdentityKey()}|${params.campaignId}`;
	const allowed = () =>
		get(can).hasAny(
			PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_READ_SCHOOL
		);
	return {
		title: 'ออกแบบเกียรติบัตร',
		template: captureRouteLoad(
			actor.then(async (user) => {
				const key = `${ownerKey()}|${params.templateId}`;
				const record =
					user?.user_type === 'staff' && allowed()
						? await getCertificateTemplate(params.templateId, { requestFetch: fetch })
						: null;
				if (record && record.campaignId !== params.campaignId)
					throw new Error('แบบเกียรติบัตรนี้ไม่ได้อยู่ในกิจกรรมตาม URL');
				return { ownerKey: key, record };
			}),
			'โหลดแบบเกียรติบัตรไม่สำเร็จ'
		)
	};
};
