import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import {
	getCorrespondenceRoutingSettings,
	getCorrespondenceOptions
} from '#lib/api/correspondence.js';
import { captureCorrespondenceRead } from '#lib/correspondence/route-data.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.PROCESS_MANAGE_SCHOOL }
};
export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	const allowed = () => get(can).has(PERMISSIONS.PROCESS_MANAGE_SCHOOL);
	return {
		title: 'ตั้งค่าการสั่งการสารบรรณ',
		settings: captureCorrespondenceRead(
			allowed,
			() => getCorrespondenceRoutingSettings({ requestFetch: fetch }),
			[]
		),
		options: captureCorrespondenceRead(
			allowed,
			() => getCorrespondenceOptions('', { requestFetch: fetch }),
			null
		)
	};
};
