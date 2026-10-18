import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { listCorrespondence, type CorrespondenceFilters } from '#lib/api/correspondence.js';
import { captureCorrespondenceRead } from '#lib/correspondence/route-data.js';

export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'ทะเบียนหนังสือ',
		icon: 'Mails',
		group: 'correspondence',
		workspace: 'operations',
		order: 100,
		user_type: 'staff',
		permission: PERMISSION_MODULES.CORRESPONDENCE
	}
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const filters: CorrespondenceFilters = {
		search: url.searchParams.get('search') ?? undefined,
		cursor: url.searchParams.get('cursor') ?? undefined
	};
	const view = url.searchParams.get('view');
	filters.view = view === 'sent' || view === 'drafts' || view === 'all' ? view : 'action_required';
	const register = url.searchParams.get('register');
	if (register === 'incoming' || register === 'outgoing' || register === 'internal')
		filters.register = register;
	const status = url.searchParams.get('status');
	if (
		status === 'draft' ||
		status === 'in_progress' ||
		status === 'revision_requested' ||
		status === 'completed' ||
		status === 'rejected' ||
		status === 'closed'
	)
		filters.status = status;
	return {
		title: 'ทะเบียนหนังสือ',
		filters,
		records: captureCorrespondenceRead(
			() => get(can).hasModule(PERMISSION_MODULES.CORRESPONDENCE),
			() => listCorrespondence(filters, { requestFetch: fetch }),
			{
				items: [],
				nextCursor: null,
				counts: { actionRequired: 0, sent: 0, drafts: 0, all: 0 },
				progress: []
			}
		)
	};
};
