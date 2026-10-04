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
import {
	getCertificateCampaign,
	listCertificateTemplates,
	listCertificateCandidates,
	type CertificateCandidateListQuery
} from '#lib/api/certificates.js';
export const load: PageLoad = ({ fetch, depends, params, url }) => {
	depends('school:app-identity');
	const actor = waitForAuthenticatedUser();
	const raw = url.searchParams.get('status');
	const status = raw === 'ready' || raw === 'needs_review' || raw === 'invalid' ? raw : undefined;
	const query: CertificateCandidateListQuery = {
		status,
		templateId: url.searchParams.get('templateId') || undefined,
		search: url.searchParams.get('search') || undefined
	};
	const filterKey =
		url.searchParams.get('status') +
		'|' +
		url.searchParams.get('templateId') +
		'|' +
		url.searchParams.get('search');
	const ownerKey = () => `${appIdentityKey()}|${params.campaignId}`;
	const allowed = () =>
		get(can).hasAny(
			PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_READ_SCHOOL
		);
	return {
		title: 'ตรวจรายชื่อผู้รับ',
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
		),
		candidates: captureRouteLoad(
			actor.then(async (user) => ({
				filterKey,
				ownerKey: ownerKey(),
				record:
					user?.user_type === 'staff' && allowed()
						? await listCertificateCandidates(params.campaignId, query, { requestFetch: fetch })
						: null
			})),
			'โหลดข้อมูลเกียรติบัตรไม่สำเร็จ'
		)
	};
};
