import type { AcademicTermChangeSet } from '#lib/api/learning-delivery.js';

export type LearningDeliveryChangeRow = {
	resourceId: string;
	changes: AcademicTermChangeSet['changes'];
	items: AcademicTermChangeSet['items'];
};

// Commands address an offering or group. A resource may have several snapshot
// differences, or a saved command that currently produces no difference.
export function buildLearningDeliveryChangeRows(
	changes: AcademicTermChangeSet['changes'],
	items: AcademicTermChangeSet['items']
): LearningDeliveryChangeRow[] {
	const rows = new Map<string, LearningDeliveryChangeRow>();
	function rowFor(resourceId: string): LearningDeliveryChangeRow {
		let row = rows.get(resourceId);
		if (!row) {
			row = { resourceId, changes: [], items: [] };
			rows.set(resourceId, row);
		}
		return row;
	}
	for (const change of changes) rowFor(change.resourceId).changes.push(change);
	for (const item of items) {
		const resourceId =
			'learningOfferingId' in item ? item.learningOfferingId : item.learningGroupId;
		rowFor(resourceId).items.push(item);
	}
	return [...rows.values()];
}
