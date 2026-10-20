import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { budgetRead } from '#lib/budget/route-data.js';
import { budgetFilters, safeReturn } from '#lib/budget/presentation.js';
import {
	getBudgetProject,
	getBudgetCatalogs,
	getBudgetProcess,
	listBudgetRequests
} from '#lib/api/budget.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSION_MODULES.BUDGET }
};
export const load: PageLoad = ({ fetch, url, params, depends }) => {
	depends('school:app-identity');
	const query = { ...budgetFilters(url), projectId: params.id };
	return {
		id: params.id,
		query,
		requestKey: params.id,
		returnTo: safeReturn(url.searchParams.get('returnTo'), '/staff/budget/projects'),
		record: budgetRead(
			() => get(can).hasModule(PERMISSION_MODULES.BUDGET),
			() => getBudgetProject(params.id, { requestFetch: fetch }),
			null
		),
		catalogs: budgetRead(
			() => get(can).hasModule(PERMISSION_MODULES.BUDGET),
			() => getBudgetCatalogs({ requestFetch: fetch }),
			null
		),
		process: budgetRead(
			() => get(can).hasModule(PERMISSION_MODULES.BUDGET),
			() => getBudgetProcess('projects', params.id, { requestFetch: fetch }),
			null
		),
		requests: budgetRead(
			() => get(can).hasModule(PERMISSION_MODULES.BUDGET),
			() => listBudgetRequests({ requestFetch: fetch, query }),
			null
		)
	};
};
