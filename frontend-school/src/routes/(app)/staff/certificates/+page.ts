import { PERMISSIONS } from '#lib/permissions/registry.js';

const managementReadPermissions = [
	PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT,
	PERMISSIONS.CERTIFICATE_READ_SCHOOL
];

export const _meta = {
	menu: {
		title: 'เกียรติบัตร',
		icon: 'Award',
		group: 'academic',
		workspace: 'academic',
		order: 60,
		user_type: 'staff',
		permission: [
			PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_READ_SCHOOL
		]
	},
	access: {
		user_type: 'staff',
		permission: managementReadPermissions
	}
};

import type { PageLoad } from './$types';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { can } from '#lib/stores/permissions.js';
import { get } from 'svelte/store';
import { listCertificateCampaigns } from '#lib/api/certificates.js';
export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	return {
		title: 'ชุดออกเกียรติบัตร',
		campaigns: captureRouteLoad(
			waitForAuthenticatedUser().then(async (user) => {
				const identityKey = appIdentityKey();
				return {
					identityKey,
					records:
						user?.user_type === 'staff' &&
						get(can).hasAny(
							PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT,
							PERMISSIONS.CERTIFICATE_READ_SCHOOL
						)
							? await listCertificateCampaigns({}, { requestFetch: fetch })
							: null
				};
			}),
			'โหลดชุดออกเกียรติบัตรไม่สำเร็จ'
		)
	};
};
