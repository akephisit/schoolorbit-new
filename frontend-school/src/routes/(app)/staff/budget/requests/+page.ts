import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { budgetRead } from '#lib/budget/route-data.js';
import { budgetFilters, safeReturn } from '#lib/budget/presentation.js';
import { listBudgetRequests } from '#lib/api/budget.js';
export const _meta = {
	academicContext: 'none' as const,
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
export const load: PageLoad = ({ fetch, url, params, depends }) => {
	depends('school:app-identity');
	const query = budgetFilters(url);
	return {
		query,
		requestKey: JSON.stringify(query),
		record: budgetRead(
			() => get(can).hasModule(PERMISSION_MODULES.BUDGET),
			() => listBudgetRequests({ requestFetch: fetch, query }),
			null
		)
	};
};
