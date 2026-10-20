import { appIdentityKey } from '#lib/auth/settled-user.js';
import {
	getBudgetDashboard,
	type BudgetAdjustmentDetail,
	type BudgetAllocationRef,
	type BudgetDashboard
} from '#lib/api/budget.js';
import type { ApiRequestOptions } from '#lib/api/client.js';
import type { RouteLoadResult } from '#lib/navigation/route-load.js';
import { canReadBudget } from './access.js';
import { budgetRead, type BudgetOwnedRead } from './route-data.js';

type OwnedSource<T> = Promise<RouteLoadResult<BudgetOwnedRead<T>>>;

/** The accepted adjustment supplies project IDs; two sides in one project share one GET. */
export function adjustmentAllocationSources(
	record: OwnedSource<BudgetAdjustmentDetail | null>,
	options: ApiRequestOptions = {}
) {
	const projects = new Map<string, OwnedSource<BudgetDashboard | null>>();
	function source(side: 'from' | 'to'): OwnedSource<BudgetDashboard | null> {
		return record.then((result) => {
			if (!result.ok) return result;
			if (result.data.identity !== appIdentityKey())
				return { ok: false, data: null, error: 'บริบทผู้ใช้เปลี่ยนแปลง กรุณาเปิดหน้านี้อีกครั้ง' };
			const ref = result.data.value?.[side];
			if (!ref)
				return { ok: true, data: { identity: result.data.identity, value: null }, error: null };
			let project = projects.get(ref.projectId);
			if (!project) {
				project = budgetRead(
					canReadBudget,
					() =>
						getBudgetDashboard({
							...options,
							query: { ...options.query, projectId: ref.projectId }
						}),
					null
				);
				projects.set(ref.projectId, project);
			}
			return project;
		});
	}
	return { from: source('from'), to: source('to') };
}

export function acceptedAllocation(
	ref: BudgetAllocationRef | null | undefined,
	dashboard: BudgetDashboard | null
) {
	if (!ref || !dashboard) return null;
	const row = dashboard.rows.find(
		(row) =>
			row.projectId === ref.projectId &&
			row.activityId === ref.activityId &&
			row.fundId === ref.fundId &&
			row.expenseCategoryId === ref.expenseCategoryId
	);
	if (
		!row?.projectName?.trim() ||
		!row.activityTitle?.trim() ||
		!row.fundName?.trim() ||
		!row.expenseCategoryName?.trim()
	)
		return null;
	return {
		projectName: row.projectName,
		activityTitle: row.activityTitle,
		fundName: row.fundName,
		categoryName: row.expenseCategoryName
	};
}
