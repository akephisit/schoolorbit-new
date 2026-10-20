import { canReadBudget, budgetRoutePermissions } from '#lib/budget/access.js';
import type { PageLoad } from './$types';
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
	access: { user_type: 'staff', permission: budgetRoutePermissions }
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
			canReadBudget,
			() => getBudgetProject(params.id, { requestFetch: fetch }),
			null
		),
		catalogs: budgetRead(canReadBudget, () => getBudgetCatalogs({ requestFetch: fetch }), null),
		process: budgetRead(
			canReadBudget,
			() => getBudgetProcess('projects', params.id, { requestFetch: fetch }),
			null
		),
		requests: budgetRead(
			canReadBudget,
			() => listBudgetRequests({ requestFetch: fetch, query }),
			null
		)
	};
};
