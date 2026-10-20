import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { budgetRead } from '#lib/budget/route-data.js';
import { safeReturn } from '#lib/budget/presentation.js';
import { getBudgetRequest, getBudgetProcess, listBudgetPayments } from '#lib/api/budget.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSION_MODULES.BUDGET }
};
export const load: PageLoad = ({ fetch, url, params, depends }) => {
	depends('school:app-identity');
	return {
		id: params.id,
		returnTo: safeReturn(url.searchParams.get('returnTo'), '/staff/budget/requests'),
		record: budgetRead(
			() => get(can).hasModule(PERMISSION_MODULES.BUDGET),
			() => getBudgetRequest(params.id, { requestFetch: fetch }),
			null
		),
		process: budgetRead(
			() => get(can).hasModule(PERMISSION_MODULES.BUDGET),
			() => getBudgetProcess('requests', params.id, { requestFetch: fetch }),
			null
		),
		payments: budgetRead(
			() => get(can).hasModule(PERMISSION_MODULES.BUDGET),
			() => listBudgetPayments(params.id, { requestFetch: fetch, query: { limit: 20, offset: 0 } }),
			null
		)
	};
};
