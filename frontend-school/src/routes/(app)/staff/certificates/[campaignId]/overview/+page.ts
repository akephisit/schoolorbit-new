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
import { getCertificateCampaign } from '$lib/api/certificates';
export const load: PageLoad = ({ fetch, depends, params }) => {
	depends('school:app-identity');
	return {
		title: 'ภาพรวมชุดออกเกียรติบัตร',
		campaign: captureRouteLoad(
			waitForAuthenticatedUser().then(async (user) => {
				const identityKey = appIdentityKey(),
					campaignId = params.campaignId;
				return {
					identityKey,
					campaignId,
					record:
						user?.user_type === 'staff' &&
						get(can).hasAny(
							PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT,
							PERMISSIONS.CERTIFICATE_READ_SCHOOL
						)
							? await getCertificateCampaign(campaignId, { requestFetch: fetch })
							: null
				};
			}),
			'โหลดกิจกรรมเกียรติบัตรไม่สำเร็จ'
		)
	};
};
