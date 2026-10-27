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

export type LearningDeliveryChangeGroup = {
	id: string;
	label: string;
	rows: LearningDeliveryChangeRow[];
};

/** Group by known resource ownership only; labels are never used as identity. */
export function groupLearningDeliveryChanges(
	rows: LearningDeliveryChangeRow[],
	labels: AcademicTermChangeSet['offeringLabels'],
	offerings: readonly {
		id: string;
		code: string;
		name: string;
		kind: 'course' | 'activity';
		groups: readonly { id: string }[];
	}[] = []
): LearningDeliveryChangeGroup[] {
	const offeringByGroup = new Map(
		offerings.flatMap((offering) =>
			offering.groups.map((group) => [group.id, offering.id] as const)
		)
	);
	const groups = new Map<string, LearningDeliveryChangeGroup>();
	for (const row of rows) {
		const item = row.items.find((item) => 'learningOfferingId' in item);
		const offeringId =
			row.changes[0]?.learningOfferingId ??
			(item && 'learningOfferingId' in item ? item.learningOfferingId : undefined) ??
			offeringByGroup.get(row.resourceId);
		const id = offeringId ?? 'teacher-commands';
		let group = groups.get(id);
		if (!group) {
			const offering = offerings.find((offering) => offering.id === offeringId);
			const label = labels.find((label) => label.id === offeringId);
			group = {
				id,
				label: offering
					? offering.kind === 'activity'
						? offering.name
						: `${offering.code} · ${offering.name}`
					: label
						? `${label.code} · ${label.name}`
						: offeringId
							? (row.changes.find((change) => change.resourceId === offeringId)?.label ??
								'รายการเปิดสอนที่ยังไม่มีชื่อ')
							: 'คำสั่งครูที่ยังไม่ได้เชื่อมกับรายการเปิดสอน',
				rows: []
			};
			groups.set(id, group);
		}
		group.rows.push(row);
	}
	return [...groups.values()].sort(
		(left, right) =>
			left.label.localeCompare(right.label, 'th-TH', { numeric: true }) ||
			left.id.localeCompare(right.id)
	);
}

export function deliveryChangeRowSearchText(row: LearningDeliveryChangeRow): string {
	return [
		...row.changes.flatMap((change) => [change.label, change.field, change.before, change.after]),
		...row.items.flatMap((item) =>
			'learningGroupLabel' in item ? [item.learningGroupLabel, item.teacherLabel] : []
		)
	]
		.filter((value) => value != null)
		.join(' ')
		.toLocaleLowerCase('th-TH');
}

export function filterLearningDeliveryChanges(
	groups: LearningDeliveryChangeGroup[],
	search: string
): LearningDeliveryChangeGroup[] {
	const query = search.trim().toLocaleLowerCase('th-TH');
	if (!query) return groups;
	return groups.flatMap((group) => {
		if (group.label.toLocaleLowerCase('th-TH').includes(query)) return [group];
		const rows = group.rows
			.filter((row) => deliveryChangeRowSearchText(row).includes(query))
			.map((row) => ({
				...row,
				changes: row.changes.filter((change) =>
					[change.label, change.field, change.before, change.after]
						.join(' ')
						.toLocaleLowerCase('th-TH')
						.includes(query)
				),
				items: row.items.filter(
					(item) =>
						'learningGroupLabel' in item &&
						`${item.learningGroupLabel} ${item.teacherLabel}`
							.toLocaleLowerCase('th-TH')
							.includes(query)
				)
			}));
		return rows.length ? [{ ...group, rows }] : [];
	});
}

export function deliveryChangePage<T>(items: readonly T[], page: number, size = 8) {
	const pageCount = Math.max(1, Math.ceil(items.length / size));
	const currentPage = Math.max(0, Math.min(page, pageCount - 1));
	return {
		items: items.slice(currentPage * size, (currentPage + 1) * size),
		page: currentPage,
		pageCount,
		total: items.length
	};
}
