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
import { listCertificateCampaignIssueRequests } from '#lib/api/certificates.js';
export const load: PageLoad = ({ fetch, depends, params }) => {
	depends('school:app-identity');
	return {
		title: 'ประวัติคำขอออกเกียรติบัตร',
		requests: captureRouteLoad(
			waitForAuthenticatedUser().then(async (user) => {
				const ownerKey = `${appIdentityKey()}|${params.campaignId}`;
				return {
					ownerKey,
					records:
						user?.user_type === 'staff' &&
						get(can).hasAny(
							PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT,
							PERMISSIONS.CERTIFICATE_READ_SCHOOL
						)
							? await listCertificateCampaignIssueRequests(params.campaignId, {
									requestFetch: fetch
								})
							: null
				};
			}),
			'โหลดข้อมูลเกียรติบัตรไม่สำเร็จ'
		)
	};
};
