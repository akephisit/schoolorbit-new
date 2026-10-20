import type { BudgetDashboardRow } from '#lib/api/budget.js';
import { sumSatang } from './money.js';

export type BudgetPivotMetric = 'planned' | 'current' | 'paid';
const fields = { planned: 'plannedSatang', current: 'currentSatang', paid: 'paidSatang' } as const;
export function budgetPivot(
	rows: readonly BudgetDashboardRow[],
	metric: BudgetPivotMetric = 'planned'
) {
	const groups: {
		id: string;
		name: string;
		columns: { key: string; categoryId: string; name: string }[];
	}[] = [];
	const activities = new Map<
		string,
		{
			key: string;
			projectId: string;
			projectName: string;
			activityId: string;
			title: string;
			values: Record<string, string>;
		}
	>();
	for (const row of rows) {
		let group = groups.find((item) => item.id === row.fundId);
		if (!group) {
			group = { id: row.fundId, name: row.fundName, columns: [] };
			groups.push(group);
		}
		const columnKey = `${row.fundId}|${row.expenseCategoryId}`;
		if (!group.columns.some((column) => column.key === columnKey))
			group.columns.push({
				key: columnKey,
				categoryId: row.expenseCategoryId,
				name: row.expenseCategoryName
			});
		const key = `${row.projectId}|${row.activityId}`;
		let activity = activities.get(key);
		if (!activity) {
			activity = {
				key,
				projectId: row.projectId,
				projectName: row.projectName,
				activityId: row.activityId,
				title: row.activityTitle,
				values: {}
			};
			activities.set(key, activity);
		}
		activity.values[columnKey] = sumSatang([
			activity.values[columnKey] ?? '0',
			row[fields[metric]]
		]);
	}
	const columns = groups.flatMap((group) => group.columns);
	const result = [...activities.values()].map((activity) => ({
		...activity,
		totalSatang: sumSatang(columns.map((column) => activity.values[column.key] ?? '0'))
	}));
	const columnTotals = Object.fromEntries(
		columns.map((column) => [
			column.key,
			sumSatang(result.map((activity) => activity.values[column.key] ?? '0'))
		])
	);
	return {
		groups,
		columns,
		activities: result,
		columnTotals,
		totalSatang: sumSatang(result.map((activity) => activity.totalSatang))
	};
}
