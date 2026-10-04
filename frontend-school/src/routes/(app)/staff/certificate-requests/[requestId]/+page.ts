import { PERMISSIONS } from '#lib/permissions/registry.js';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.CERTIFICATE_ISSUE_SCHOOL
	}
};

import type { PageLoad } from './$types';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { can } from '#lib/stores/permissions.js';
import { get } from 'svelte/store';
import { getCertificateIssueRequest } from '#lib/api/certificates.js';
export const load: PageLoad = ({ fetch, depends, params }) => {
	depends('school:app-identity');
	return {
		title: 'ตรวจคำขอออกเกียรติบัตร',
		request: captureRouteLoad(
			waitForAuthenticatedUser().then(async (user) => {
				const ownerKey = `${appIdentityKey()}|${params.requestId}`;
				return {
					ownerKey,
					record:
						user?.user_type === 'staff' && get(can).has(PERMISSIONS.CERTIFICATE_ISSUE_SCHOOL)
							? await getCertificateIssueRequest(params.requestId, { requestFetch: fetch })
							: null
				};
			}),
			'โหลดข้อมูลเกียรติบัตรไม่สำเร็จ'
		)
	};
};
