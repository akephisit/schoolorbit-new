import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { getCorrespondence } from '#lib/api/correspondence.js';
import { captureCorrespondenceRead } from '#lib/correspondence/route-data.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSION_MODULES.CORRESPONDENCE }
};
export const load: PageLoad = ({ fetch, params, depends }) => {
	depends('school:app-identity');
	return {
		title: 'รายละเอียดหนังสือ',
		id: params.id,
		record: captureCorrespondenceRead(
			() => get(can).hasModule(PERMISSION_MODULES.CORRESPONDENCE),
			() => getCorrespondence(params.id, { requestFetch: fetch }),
			null
		)
	};
};
