import type {
	CurriculumPreparationChoice,
	CurriculumPreparationProposal,
	HomeroomDeliveryItem,
	HomeroomDeliveryWorkspace,
	DeliveryVersionSummary
} from '#lib/api/learning-delivery.js';

export type CurriculumPreparationFocus = Pick<
	CurriculumPreparationProposal,
	'resourceKind' | 'catalogVersionId'
>;

export type CurriculumPreparationTarget = CurriculumPreparationFocus & {
	code: string;
	name: string;
	schedulingMode: HomeroomDeliveryItem['schedulingMode'];
	studyProgramIds: string[];
	homeroomCount: number;
};

export function isPendingSynchronizedActivity(item: HomeroomDeliveryItem): boolean {
	return (
		item.resourceKind === 'activity' &&
		item.schedulingMode === 'synchronized' &&
		item.offeringId === null
	);
}

export function buildCurriculumPreparationTarget(
	workspace: Pick<HomeroomDeliveryWorkspace, 'homerooms'>,
	focus: CurriculumPreparationFocus
): CurriculumPreparationTarget | null {
	const matches = (item: HomeroomDeliveryItem) =>
		item.resourceKind === focus.resourceKind && item.catalogVersionId === focus.catalogVersionId;
	const matchingRooms = workspace.homerooms.filter((room) =>
		room.items.some((item) => matches(item) && !item.offeringId)
	);
	const pending = matchingRooms
		.flatMap((room) => room.items)
		.find((item) => matches(item) && !item.offeringId);
	if (!pending) return null;
	return {
		...focus,
		code: pending.code,
		name: pending.name,
		schedulingMode: pending.schedulingMode,
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
	const canApplyDefaults =
		proposal.schedulingMode !== 'synchronized' &&
		proposal.groupingState === 'proposed' &&
		proposal.conflicts.length === 0;
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

/** Only drafts based on the latest published source can receive this intent. */
export function deliveryPreparationDraftCandidates(versions: DeliveryVersionSummary[]) {
	const published = versions
		.filter((version) => version.status === 'published')
		.sort(
			(left, right) =>
				(right.effectiveFrom ?? '').localeCompare(left.effectiveFrom ?? '') ||
				right.id.localeCompare(left.id)
		)[0];
	return {
		source: published ?? null,
		drafts: published
			? versions.filter(
					(version) => version.status === 'draft' && version.sourceVersionId === published.id
				)
			: []
	};
}
