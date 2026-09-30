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

import type { PageLoad } from './$types';
import { appIdentityKey, waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { can } from '$lib/stores/permissions';
import { get } from 'svelte/store';
import { getCertificateCampaign, listCertificateTemplates } from '$lib/api/certificates';
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
		title: 'แบบเกียรติบัตร',
		campaign: captureRouteLoad(
			actor.then(async (user) => ({
				ownerKey: ownerKey(),
				record:
					user?.user_type === 'staff' && allowed()
						? await getCertificateCampaign(params.campaignId, { requestFetch: fetch })
						: null
			})),
			'โหลดข้อมูลเกียรติบัตรไม่สำเร็จ'
		),
		templates: captureRouteLoad(
			actor.then(async (user) => ({
				ownerKey: ownerKey(),
				record:
					user?.user_type === 'staff' && allowed()
						? await listCertificateTemplates(params.campaignId, { requestFetch: fetch })
						: null
			})),
			'โหลดข้อมูลเกียรติบัตรไม่สำเร็จ'
		)
	};
};
