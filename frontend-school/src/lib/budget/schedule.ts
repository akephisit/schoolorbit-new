import type { BudgetActivity } from '#lib/api/budget.js';
export function activityMonths(activities: readonly BudgetActivity[]): string[] {
	const months = new Set<string>();
	for (const activity of activities) {
		if (!activity.startDate || !activity.endDate) continue;
		let month = activity.startDate.slice(0, 7);
		const end = activity.endDate.slice(0, 7);
		while (month <= end) {
			months.add(month);
			const [year, number] = month.split('-').map(Number);
			month = number === 12 ? `${year + 1}-01` : `${year}-${String(number + 1).padStart(2, '0')}`;
		}
	}
	return [...months].sort();
}
export function activitiesInMonth(
	activities: readonly BudgetActivity[],
	month: string
): BudgetActivity[] {
	const start = `${month}-01`;
	const [year, number] = month.split('-').map(Number);
	const end = new Date(Date.UTC(year, number, 0)).toISOString().slice(0, 10);
	return activities.filter(
		(activity) =>
			activity.startDate &&
			activity.endDate &&
			activity.startDate <= end &&
			activity.endDate >= start
	);
}
export function monthLabel(month: string): string {
	return new Date(`${month}-01T00:00:00Z`).toLocaleDateString('th-TH', {
		timeZone: 'UTC',
		month: 'long',
		year: 'numeric'
	});
}
