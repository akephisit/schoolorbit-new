import { canReadBudget, budgetRoutePermissions } from '#lib/budget/access.js';
import type { PageLoad } from './$types';
import { budgetRead } from '#lib/budget/route-data.js';
import { budgetFilters, safeReturn } from '#lib/budget/presentation.js';
import { getBudgetAdjustment, getBudgetProcess, getBudgetCatalogs } from '#lib/api/budget.js';
import { adjustmentAllocationSources } from '#lib/budget/adjustment-context.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: budgetRoutePermissions }
};
export const load: PageLoad = ({ fetch, url, params, depends }) => {
	depends('school:app-identity');
	const query = budgetFilters(url);
	const record = budgetRead(
		canReadBudget,
		() => getBudgetAdjustment(params.id, { requestFetch: fetch }),
		null
	);
	const allocations = adjustmentAllocationSources(record, { requestFetch: fetch });
	return {
		requestKey: JSON.stringify(query) + ('id' in params ? params.id : ''),
		id: params.id,
		returnTo: safeReturn(url.searchParams.get('returnTo'), '/staff/budget/adjustments'),
		record,
		fromBudget: allocations.from,
		toBudget: allocations.to,
		catalogs: budgetRead(canReadBudget, () => getBudgetCatalogs({ requestFetch: fetch }), null),
		process: budgetRead(
			canReadBudget,
			() => getBudgetProcess('adjustments', params.id, { requestFetch: fetch }),
			null
		)
	};
};
