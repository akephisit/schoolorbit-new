import type { DeliveryVersion, LearningGroup } from '#lib/api/learning-delivery.js';
import type { DeliveryOverviewState } from '#lib/navigation/learning-delivery.js';

const naturalOrder = new Intl.Collator('th-TH', { numeric: true });
type Offering = DeliveryVersion['snapshot']['offerings'][number];

export function deliveryOfferingLabel(item: Pick<Offering, 'kind' | 'code' | 'name'>): string {
	return item.kind === 'activity' ? item.name : `${item.code} · ${item.name}`;
}

export function compareDeliveryOfferings(
	left: Pick<Offering, 'id' | 'kind' | 'code' | 'name'>,
	right: Pick<Offering, 'id' | 'kind' | 'code' | 'name'>
): number {
	return (
		(left.kind === 'course' ? 0 : 1) - (right.kind === 'course' ? 0 : 1) ||
		(left.kind === 'course' ? naturalOrder.compare(left.code, right.code) : 0) ||
		naturalOrder.compare(left.name, right.name) ||
		left.id.localeCompare(right.id)
	);
}

export function deliverySubjectGroupOptions(
	version: DeliveryVersion
): { id: string; name: string }[] {
	const pinnedIds = new Set(
		version.snapshot.offerings.flatMap((item) =>
			item.catalog.kind === 'course' ? [item.catalog.subjectVersionId] : []
		)
	);
	const groups = new Map<string, string>();
	for (const metadata of version.courseMetadata) {
		if (pinnedIds.has(metadata.subjectVersionId))
			groups.set(metadata.subjectGroupId, metadata.subjectGroupName);
	}
	return [...groups]
		.map(([id, name]) => ({ id, name }))
		.sort(
			(left, right) =>
				naturalOrder.compare(left.name, right.name) || left.id.localeCompare(right.id)
		);
}

export function filterDeliveryOfferings(
	version: DeliveryVersion,
	filters: Pick<DeliveryOverviewState, 'kind' | 'offeringSearch' | 'subjectGroupId'>
): Offering[] {
	const groups = new Map(
		version.courseMetadata.map((item) => [item.subjectVersionId, item.subjectGroupId])
	);
	const search = filters.offeringSearch.trim().toLocaleLowerCase('th-TH');
	return version.snapshot.offerings
		.filter((item) => {
			if (filters.kind !== 'all' && item.kind !== filters.kind) return false;
			if (
				filters.subjectGroupId !== 'all' &&
				(item.catalog.kind !== 'course' ||
					groups.get(item.catalog.subjectVersionId) !== filters.subjectGroupId)
			)
				return false;
			return !search || deliveryOfferingLabel(item).toLocaleLowerCase('th-TH').includes(search);
		})
		.sort(compareDeliveryOfferings);
}

export type DeliveryGroupCoverage = Pick<
	DeliveryVersion['snapshot']['offerings'][number]['groups'][number],
	'id' | 'homeroomIds'
>;

/** A selected version supplies presentation coverage, never a replacement registry mutation payload. */
export function deliveryGroupCoverage(
	group: Pick<LearningGroup, 'id' | 'homeroomIds'>,
	selectedGroups: readonly DeliveryGroupCoverage[] | null
) {
	const selected = selectedGroups?.find((item) => item.id === group.id);
	const homeroomIds = selectedGroups === null ? group.homeroomIds : (selected?.homeroomIds ?? []);
	const registryIds = new Set(group.homeroomIds);
	const selectedIds = new Set(homeroomIds);
	return {
		homeroomIds: [...homeroomIds],
		differsFromRegistry:
			selectedGroups !== null &&
			(selectedIds.size !== registryIds.size || [...selectedIds].some((id) => !registryIds.has(id)))
	};
}
