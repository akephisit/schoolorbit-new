import { canReadBudget, budgetRoutePermissions } from '#lib/budget/access.js';
import type { PageLoad } from './$types';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { budgetRead } from '#lib/budget/route-data.js';
import { budgetFilters } from '#lib/budget/presentation.js';
import { listBudgetAdjustments } from '#lib/api/budget.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: budgetRoutePermissions },
	menu: {
		title: 'ปรับงบประมาณและเงินยืม',
		icon: 'ArrowLeftRight',
		workspace: 'budget',
		group: 'budget_finance',
		order: 110,
		user_type: 'staff',
		permission: PERMISSION_MODULES.BUDGET
	}
};
export const load: PageLoad = ({ fetch, url, params, depends }) => {
	depends('school:app-identity');
	const query = budgetFilters(url);
	return {
		requestKey: JSON.stringify(query) + ('id' in params ? params.id : ''),
		query,
		record: budgetRead(
			canReadBudget,
			() => listBudgetAdjustments({ requestFetch: fetch, query }),
			null
		)
	};
};
