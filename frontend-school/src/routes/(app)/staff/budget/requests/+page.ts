import { canReadBudget, budgetRoutePermissions } from '#lib/budget/access.js';
import type { PageLoad } from './$types';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { budgetRead } from '#lib/budget/route-data.js';
import { budgetFilters } from '#lib/budget/presentation.js';
import { listBudgetRequests } from '#lib/api/budget.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: budgetRoutePermissions },
	menu: {
		title: 'คำขอใช้เงิน',
		icon: 'Receipt',
		group: 'budget_finance',
		workspace: 'budget',
		order: 100,
		user_type: 'staff',
		permission: PERMISSION_MODULES.BUDGET
	}
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const query = budgetFilters(url);
	return {
		query,
		requestKey: JSON.stringify(query),
		record: budgetRead(
			canReadBudget,
			() => listBudgetRequests({ requestFetch: fetch, query }),
			null
		)
	};
};
