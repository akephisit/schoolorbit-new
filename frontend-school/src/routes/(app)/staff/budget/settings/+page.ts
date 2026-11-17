import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { budgetRead } from '#lib/budget/route-data.js';
import { getBudgetSettings, getBudgetYearSettings } from '#lib/api/budget.js';
export const _meta = {
	academicContext: 'none' as const,
	access: {
		user_type: 'staff',
		permission: [PERMISSIONS.BUDGET_MANAGE_SCHOOL, PERMISSIONS.BUDGET_PAYMENT_EXECUTE_SCHOOL]
	}
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const selectedId = url.searchParams.get('fiscalYearId') ?? '';
	const allowed = () =>
		get(can).hasAny(PERMISSIONS.BUDGET_MANAGE_SCHOOL, PERMISSIONS.BUDGET_PAYMENT_EXECUTE_SCHOOL);
	const index = budgetRead(allowed, () => getBudgetSettings({ requestFetch: fetch }), null);
	return {
		selectedId,
		index,
		record: budgetRead(
			allowed,
			async () => {
				const result = await index;
				if (!result.ok || !result.data.value) return null;
				const years = result.data.value.fiscalYears;
				const id = selectedId || years.find((year) => !year.isClosed)?.id || years[0]?.id;
				return id ? getBudgetYearSettings(id, { requestFetch: fetch }) : null;
			},
			null
		)
	};
};
