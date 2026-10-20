import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { budgetRead } from '#lib/budget/route-data.js';
import { budgetFilters } from '#lib/budget/presentation.js';
import { getBudgetSettings } from '#lib/api/budget.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.BUDGET_MANAGE_SCHOOL }
};
export const load: PageLoad = ({ fetch, url, params, depends }) => {
	depends('school:app-identity');
	const query = budgetFilters(url);
	return {
		requestKey: JSON.stringify(query) + ('id' in params ? params.id : ''),
		record: budgetRead(
			() => get(can).has(PERMISSIONS.BUDGET_MANAGE_SCHOOL),
			() => getBudgetSettings({ requestFetch: fetch }),
			null
		)
	};
};
