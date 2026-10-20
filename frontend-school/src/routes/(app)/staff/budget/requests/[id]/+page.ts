import { canReadBudget, budgetRoutePermissions } from '#lib/budget/access.js';
import type { PageLoad } from './$types';
import { budgetRead } from '#lib/budget/route-data.js';
import { safeReturn } from '#lib/budget/presentation.js';
import { getBudgetRequest, getBudgetProcess, listBudgetPayments } from '#lib/api/budget.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: budgetRoutePermissions }
};
export const load: PageLoad = ({ fetch, url, params, depends }) => {
	depends('school:app-identity');
	return {
		id: params.id,
		returnTo: safeReturn(url.searchParams.get('returnTo'), '/staff/budget/requests'),
		record: budgetRead(
			canReadBudget,
			() => getBudgetRequest(params.id, { requestFetch: fetch }),
			null
		),
		process: budgetRead(
			canReadBudget,
			() => getBudgetProcess('requests', params.id, { requestFetch: fetch }),
			null
		),
		payments: budgetRead(
			canReadBudget,
			() => listBudgetPayments(params.id, { requestFetch: fetch, query: { limit: 20, offset: 0 } }),
			null
		)
	};
};
