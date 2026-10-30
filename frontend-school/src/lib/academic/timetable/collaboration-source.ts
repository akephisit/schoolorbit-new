import type {
	TimetableBlockPlacementSource,
	TimetableBlockWorkspace
} from '../../api/timetable.js';

/** Identity shared by cards in different views; row versions are validated separately. */
export function placementSourceKey(source: TimetableBlockPlacementSource): string {
	switch (source.kind) {
		case 'existing_block':
			return `block:${source.blockId}`;
		case 'ordinary_demand':
			return `group:${source.learningGroupId}`;
		case 'synchronized_offering':
			return `offering:${source.learningOfferingId}`;
	}
}

export function sourceGhost(
	workspace: Pick<
		TimetableBlockWorkspace,
		'blocks' | 'ordinaryDemands' | 'learningGroups' | 'synchronizedDemands'
	>,
	source: TimetableBlockPlacementSource
): { code: string | null; title: string } | null {
	if (source.kind === 'existing_block') {
		const block = workspace.blocks.find((item) => item.id === source.blockId);
		return block
			? {
					code: block.blockKind === 'course' ? block.offeringCode : null,
					title: block.offeringName ?? block.title ?? 'รายการตารางสอน'
				}
			: null;
	}
	if (source.kind === 'ordinary_demand') {
		const demand = workspace.ordinaryDemands.find(
			(item) => item.learningGroupId === source.learningGroupId
		);
		const group = workspace.learningGroups.find((item) => item.id === source.learningGroupId);
		return demand
			? {
					code: group?.offeringKind === 'course' ? demand.offeringCode : null,
					title: demand.offeringName
				}
			: null;
	}
	const demand = workspace.synchronizedDemands.find(
		(item) => item.learningOfferingId === source.learningOfferingId
	);
	return demand ? { code: null, title: demand.offeringName } : null;
}
