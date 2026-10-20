import { appIdentityKey } from '#lib/auth/settled-user.js';
import {
	getBudgetProject,
	type BudgetAdjustmentDetail,
	type BudgetAllocationRef,
	type BudgetCatalogs,
	type BudgetProjectWorkspace
} from '#lib/api/budget.js';
import type { ApiRequestOptions } from '#lib/api/client.js';
import type { RouteLoadResult } from '#lib/navigation/route-load.js';
import { canReadBudget } from './access.js';
import { budgetRead, type BudgetOwnedRead } from './route-data.js';

type OwnedSource<T> = Promise<RouteLoadResult<BudgetOwnedRead<T>>>;

/** The accepted adjustment supplies project IDs; two sides in one project share one GET. */
export function adjustmentProjectSources(
	record: OwnedSource<BudgetAdjustmentDetail | null>,
	options: ApiRequestOptions = {}
) {
	const projects = new Map<string, OwnedSource<BudgetProjectWorkspace | null>>();
	function source(side: 'from' | 'to'): OwnedSource<BudgetProjectWorkspace | null> {
		return record.then((result) => {
			if (!result.ok) return result;
			if (result.data.identity !== appIdentityKey())
				return { ok: false, data: null, error: 'บริบทผู้ใช้เปลี่ยนแปลง กรุณาเปิดหน้านี้อีกครั้ง' };
			const ref = result.data.value?.[side];
			if (!ref)
				return { ok: true, data: { identity: result.data.identity, value: null }, error: null };
			let project = projects.get(ref.projectId);
			if (!project) {
				project = budgetRead(canReadBudget, () => getBudgetProject(ref.projectId, options), null);
				projects.set(ref.projectId, project);
			}
			return project;
		});
	}
	return { from: source('from'), to: source('to') };
}

export function acceptedAllocation(
	ref: BudgetAllocationRef | null | undefined,
	fiscalYearId: string,
	project: BudgetProjectWorkspace | null,
	catalogs: BudgetCatalogs | null
) {
	if (
		!ref ||
		!project ||
		project.project.id !== ref.projectId ||
		!project.acceptedPlan ||
		project.acceptedPlan.project.fiscalYearId !== fiscalYearId
	)
		return null;
	const activity = project.acceptedPlan.activities.find(
		(activity) => activity.id === ref.activityId
	);
	const fund = catalogs?.funds.find((fund) => fund.id === ref.fundId);
	const category = catalogs?.expenseCategories.find(
		(category) => category.id === ref.expenseCategoryId
	);
	if (!activity || !fund || !category) return null;
	return {
		projectName: project.acceptedPlan.project.name,
		activityTitle: activity.title,
		fundName: fund.name,
		categoryName: category.name
	};
}
