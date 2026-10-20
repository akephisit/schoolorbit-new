import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { budgetRead } from '#lib/budget/route-data.js';
import { budgetFilters, safeReturn } from '#lib/budget/presentation.js';
import { getBudgetDashboard, getBudgetCatalogs } from '#lib/api/budget.js';
export const _meta = {
	academicContext: 'none' as const,
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
export const load: PageLoad = ({ fetch, url, params, depends }) => {
	depends('school:app-identity');
	const filters = budgetFilters(url);
	const query = { fiscalYearId: filters.fiscalYearId || undefined };
	return {
		query,
		requestKey: JSON.stringify(query),
		record: budgetRead(
			() => get(can).hasModule(PERMISSION_MODULES.BUDGET),
			() => getBudgetDashboard({ requestFetch: fetch, query }),
			null
		),
		catalogs: budgetRead(
			() => get(can).hasModule(PERMISSION_MODULES.BUDGET),
			() => getBudgetCatalogs({ requestFetch: fetch }),
			null
		)
	};
};
