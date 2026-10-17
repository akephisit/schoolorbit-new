import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { listProcessTemplates } from '#lib/api/correspondence.js';
import { captureCorrespondenceRead } from '#lib/correspondence/route-data.js';
export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'แม่แบบงานสารบรรณ',
		icon: 'ListOrdered',
		group: 'correspondence',
		workspace: 'operations',
		order: 130,
		user_type: 'staff',
		permission: PERMISSIONS.PROCESS_MANAGE_SCHOOL
	},
	access: { user_type: 'staff', permission: PERMISSIONS.PROCESS_MANAGE_SCHOOL }
};
export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	return {
		title: 'แม่แบบงานสารบรรณ',
		templates: captureCorrespondenceRead(
			() => get(can).has(PERMISSIONS.PROCESS_MANAGE_SCHOOL),
			() => listProcessTemplates({ requestFetch: fetch }),
			[]
		)
	};
};
