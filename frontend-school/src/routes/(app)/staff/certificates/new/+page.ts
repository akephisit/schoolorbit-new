import { PERMISSIONS } from '#lib/permissions/registry.js';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: [
			PERMISSIONS.CERTIFICATE_CREATE_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_CREATE_SCHOOL
		]
	}
};

import type { PageLoad } from './$types';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { can } from '#lib/stores/permissions.js';
import { get } from 'svelte/store';
import { listCertificateOwnerOptions } from '#lib/api/certificates.js';
import { lookupAcademicYears } from '#lib/api/lookup.js';
export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	const settled = waitForAuthenticatedUser();
	return {
		title: 'สร้างกิจกรรมเกียรติบัตร',
		years: captureRouteLoad(
			settled.then(async (user) => {
				const identityKey = appIdentityKey();
				return {
					identityKey,
					records:
						user?.user_type === 'staff' &&
						get(can).hasAny(
							PERMISSIONS.CERTIFICATE_CREATE_ORGANIZATION_UNIT,
							PERMISSIONS.CERTIFICATE_CREATE_SCHOOL
						)
							? await lookupAcademicYears({ activeOnly: false }, { requestFetch: fetch })
							: null
				};
			}),
			'โหลดปีการศึกษาไม่สำเร็จ'
		),
		owners: captureRouteLoad(
			settled.then(async (user) => {
				const identityKey = appIdentityKey();
				return {
					identityKey,
					records:
						user?.user_type === 'staff' &&
						get(can).hasAny(
							PERMISSIONS.CERTIFICATE_CREATE_ORGANIZATION_UNIT,
							PERMISSIONS.CERTIFICATE_CREATE_SCHOOL
						)
							? await listCertificateOwnerOptions({ requestFetch: fetch })
							: null
				};
			}),
			'โหลดหน่วยงานเจ้าของไม่สำเร็จ'
		)
	};
};
