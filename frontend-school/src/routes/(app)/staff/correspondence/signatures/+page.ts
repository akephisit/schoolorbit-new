import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { listSignatureAssets } from '#lib/api/correspondence.js';
import { captureCorrespondenceRead } from '#lib/correspondence/route-data.js';
export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'ลายมือชื่อของฉัน',
		icon: 'Signature',
		group: 'correspondence',
		workspace: 'operations',
		order: 120,
		user_type: 'staff',
		permission: PERMISSIONS.CORRESPONDENCE_SIGN_OWN
	},
	access: { user_type: 'staff', permission: PERMISSIONS.CORRESPONDENCE_SIGN_OWN }
};
export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	return {
		title: 'ลายมือชื่อของฉัน',
		signatures: captureCorrespondenceRead(
			() => get(can).has(PERMISSIONS.CORRESPONDENCE_SIGN_OWN),
			() => listSignatureAssets({ requestFetch: fetch }),
			[]
		)
	};
};
