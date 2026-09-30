import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'คำขอออกเกียรติบัตร',
		icon: 'ClipboardCheck',
		group: 'academic',
		workspace: 'academic',
		order: 61,
		user_type: 'staff',
		permission: PERMISSIONS.CERTIFICATE_ISSUE_SCHOOL
	},
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.CERTIFICATE_ISSUE_SCHOOL
	}
};

import type { PageLoad } from './$types';
import { appIdentityKey, waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { can } from '$lib/stores/permissions';
import { get } from 'svelte/store';
import {
	listCertificateIssueRequests,
	type CertificateIssueRequestStatus
} from '$lib/api/certificates';
export const load: PageLoad = ({ fetch, depends, url }) => {
	depends('school:app-identity');
	const raw = url.searchParams.get('status');
	const status: CertificateIssueRequestStatus | 'all' =
		raw === 'pending' ||
		raw === 'reviewing' ||
		raw === 'returned' ||
		raw === 'withdrawn' ||
		raw === 'issued'
			? raw
			: 'all';

	return {
		title: 'คิวตรวจคำขอออกเกียรติบัตร',
		status,
		requests: captureRouteLoad(
			waitForAuthenticatedUser().then(async (user) => {
				const ownerKey = `${appIdentityKey()}|${status}`;
				return {
					ownerKey,
					records:
						user?.user_type === 'staff' && get(can).has(PERMISSIONS.CERTIFICATE_ISSUE_SCHOOL)
							? await listCertificateIssueRequests(
									{ status: status === 'all' ? undefined : status },
									{ requestFetch: fetch }
								)
							: null
				};
			}),
			'โหลดข้อมูลเกียรติบัตรไม่สำเร็จ'
		)
	};
};
