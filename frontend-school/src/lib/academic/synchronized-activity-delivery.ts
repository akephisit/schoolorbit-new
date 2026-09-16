import type {
	CurriculumPreparationChoice,
	CurriculumPreparationProposal,
	HomeroomDeliveryItem,
	HomeroomDeliveryWorkspace
} from '$lib/api/learning-delivery';

export type CurriculumPreparationFocus = Pick<
	CurriculumPreparationProposal,
	'resourceKind' | 'catalogVersionId'
>;

export type SynchronizedActivityPreparationTarget = CurriculumPreparationFocus & {
	code: string;
	name: string;
	studyProgramIds: string[];
	homeroomCount: number;
};

export type DeliveryTimetableAction =
	| 'none'
	| 'activate'
	| 'revise_then_activate'
	| 'include'
	| 'revise_then_include';

export function deliveryTimetableAction(
	item: HomeroomDeliveryItem,
	versionStatus: HomeroomDeliveryWorkspace['timetableVersionStatus']
): DeliveryTimetableAction {
	if (isPendingSynchronizedActivity(item)) {
		return versionStatus === 'published' ? 'revise_then_activate' : 'activate';
	}
	if (item.offeringId !== null && item.weeklyPeriodTarget === null) {
		if (versionStatus === 'published') return 'revise_then_include';
		if (versionStatus === 'draft') return 'include';
	}
	return 'none';
}

function isSynchronizedActivity(item: HomeroomDeliveryItem, catalogVersionId: string): boolean {
	return (
		item.resourceKind === 'activity' &&
		item.catalogVersionId === catalogVersionId &&
		item.schedulingMode === 'synchronized'
	);
}

export function isPendingSynchronizedActivity(item: HomeroomDeliveryItem): boolean {
	return (
		item.resourceKind === 'activity' &&
		item.schedulingMode === 'synchronized' &&
		item.offeringId === null
	);
}

export function buildSynchronizedActivityPreparationTarget(
	workspace: Pick<HomeroomDeliveryWorkspace, 'homerooms'>,
	catalogVersionId: string
): SynchronizedActivityPreparationTarget | null {
	const matchingRooms = workspace.homerooms.filter((room) =>
		room.items.some((item) => isSynchronizedActivity(item, catalogVersionId))
	);
	const pendingItem = matchingRooms
		.flatMap((room) => room.items)
		.find((item) => isSynchronizedActivity(item, catalogVersionId) && item.offeringId === null);
	if (!pendingItem) return null;

	return {
		resourceKind: 'activity',
		catalogVersionId,
		code: pendingItem.code,
		name: pendingItem.name,
		studyProgramIds: [...new Set(matchingRooms.map((room) => room.studyProgram.id))].sort(
			(left, right) => left.localeCompare(right)
		),
		homeroomCount: matchingRooms.length
	};
}

function matchesFocus(
	proposal: CurriculumPreparationProposal,
	focus: CurriculumPreparationFocus
): boolean {
	return (
		proposal.resourceKind === focus.resourceKind &&
		proposal.catalogVersionId === focus.catalogVersionId
	);
}

function initialChoice(proposal: CurriculumPreparationProposal): CurriculumPreparationChoice {
	const canApplyDefaults = proposal.groupingState === 'proposed' && proposal.conflicts.length === 0;
	return {
		proposalId: proposal.proposalId,
		action: canApplyDefaults ? 'apply' : 'defer_groups',
		groups: canApplyDefaults
			? proposal.defaultGroups.map((group) => ({
					...group,
					homeroomIds: [...group.homeroomIds]
				}))
			: []
	};
}

export function buildFocusedCurriculumPreparationChoices(
	proposals: CurriculumPreparationProposal[],
	focus: CurriculumPreparationFocus | null
): CurriculumPreparationChoice[] {
	return proposals.map((proposal) =>
		focus && !matchesFocus(proposal, focus)
			? { proposalId: proposal.proposalId, action: 'skip', groups: [] }
			: initialChoice(proposal)
	);
}

export function visibleCurriculumPreparationProposals(
	proposals: CurriculumPreparationProposal[],
	focus: CurriculumPreparationFocus | null
): CurriculumPreparationProposal[] {
	return focus ? proposals.filter((proposal) => matchesFocus(proposal, focus)) : proposals;
}
