import { canReadBudget, budgetRoutePermissions } from '#lib/budget/access.js';
import type { PageLoad } from './$types';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { budgetRead } from '#lib/budget/route-data.js';
import { budgetFilters } from '#lib/budget/presentation.js';
import { getBudgetDashboard, getBudgetCatalogs } from '#lib/api/budget.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: budgetRoutePermissions },
	menu: {
		title: 'รายงานงบประมาณ',
		icon: 'ChartNoAxesCombined',
		group: 'budget_planning',
		workspace: 'budget',
		order: 120,
		user_type: 'staff',
		permission: PERMISSION_MODULES.BUDGET
	}
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const filters = budgetFilters(url);
	const query = { fiscalYearId: filters.fiscalYearId || undefined };
	return {
		query,
		requestKey: JSON.stringify(query),
		record: budgetRead(
			canReadBudget,
			() => getBudgetDashboard({ requestFetch: fetch, query }),
			null
		),
		catalogs: budgetRead(canReadBudget, () => getBudgetCatalogs({ requestFetch: fetch }), null)
	};
};
