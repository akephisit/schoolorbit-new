import { PERMISSIONS } from '#lib/permissions/registry.js';

export const ssr = false;

export const _meta = {
	menu: {
		title: 'เกียรติบัตรของฉัน',
		icon: 'Award',
		group: 'main',
		workspace: 'home',
		order: 6,
		user_type: 'student',
		permission: PERMISSIONS.CERTIFICATE_READ_OWN
	},
	access: {
		user_type: 'student',
		permission: PERMISSIONS.CERTIFICATE_READ_OWN
	}
};

import type { PageLoad } from './$types';
import { listOwnCertificates } from '#lib/api/certificates.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { can } from '#lib/stores/permissions.js';
import { get } from 'svelte/store';
export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	return {
		title: 'เกียรติบัตรของฉัน',
		certificates: captureRouteLoad(
			waitForAuthenticatedUser().then(async (user) => {
				const allowed =
					user?.user_type === 'student' && get(can).has(PERMISSIONS.CERTIFICATE_READ_OWN);
				return {
					ownerKey: `${user?.id ?? ''}|${allowed}`,
					records: allowed ? await listOwnCertificates({ requestFetch: fetch }) : null
				};
			}),
			'โหลดคลังเกียรติบัตรไม่สำเร็จ'
		)
	};
};
