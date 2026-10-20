import type {
	CurriculumManagementOptions,
	CurriculumStructureRequirement,
	CurriculumStructureRequirementInput
} from '#lib/api/academic-core.js';

export type CurriculumCatalogOption = CurriculumManagementOptions['catalogVersions'][number];
export type StagedCurriculumRequirement = CurriculumStructureRequirementInput & {
	studyProgramId: string;
};
export type CurriculumSelectionContext = {
	studyProgramId: string;
	gradeLevelId: string;
	termSlotId: string;
};

export function selectableCatalogOption(option: CurriculumCatalogOption): boolean {
	return option.selectableForNewRequirement;
}

function requirementMetadata(
	requirement: StagedCurriculumRequirement,
	options: CurriculumCatalogOption[],
	originals: CurriculumStructureRequirement[]
) {
	return (
		originals.find(
			(source) =>
				source.resourceKind === requirement.resourceKind &&
				source.catalogVersionId === requirement.catalogVersionId
		) ??
		options.find(
			(option) =>
				option.resourceKind === requirement.resourceKind &&
				option.id === requirement.catalogVersionId
		)
	);
}

function duplicateRequirements(
	option: CurriculumCatalogOption,
	context: CurriculumSelectionContext,
	requirements: StagedCurriculumRequirement[],
	options: CurriculumCatalogOption[],
	originals: CurriculumStructureRequirement[]
) {
	return requirements.filter((requirement) => {
		if (
			requirement.resourceKind !== option.resourceKind ||
			requirement.studyProgramId !== context.studyProgramId ||
			requirement.gradeLevelId !== context.gradeLevelId ||
			requirement.termSlotId !== context.termSlotId
		)
			return false;
		if (option.resourceKind === 'activity') return requirement.catalogVersionId === option.id;
		return requirementMetadata(requirement, options, originals)?.catalogId === option.catalogId;
	});
}

export function existingCourseVersions(
	option: CurriculumCatalogOption,
	context: CurriculumSelectionContext,
	requirements: StagedCurriculumRequirement[],
	options: CurriculumCatalogOption[],
	originals: CurriculumStructureRequirement[]
): number[] {
	if (option.resourceKind !== 'course') return [];
	return [
		...new Set(
			duplicateRequirements(option, context, requirements, options, originals)
				.map((requirement) => requirementMetadata(requirement, options, originals)?.versionNo)
				.filter((version): version is number => version !== undefined)
		)
	];
}

// This guards local selection changes; the save endpoint remains authoritative for freshness.
export function appendSelectedCatalogRequirements(
	requirements: StagedCurriculumRequirement[],
	selectedIds: string[],
	context: CurriculumSelectionContext,
	options: CurriculumCatalogOption[],
	originals: CurriculumStructureRequirement[],
	requirementKind: CurriculumStructureRequirementInput['requirementKind']
): StagedCurriculumRequirement[] {
	if (!context.studyProgramId || !context.gradeLevelId || !context.termSlotId) return requirements;
	const next = [...requirements];
	let order =
		Math.max(
			0,
			...next
				.filter(
					(requirement) =>
						requirement.studyProgramId === context.studyProgramId &&
						requirement.gradeLevelId === context.gradeLevelId &&
						requirement.termSlotId === context.termSlotId
				)
				.map((requirement) => requirement.displayOrder)
		) + 1;
	for (const id of selectedIds) {
		const option = options.find((candidate) => candidate.id === id);
		if (
			!option ||
			!selectableCatalogOption(option) ||
			duplicateRequirements(option, context, next, options, originals).length > 0
		)
			continue;
		next.push({
			...context,
			resourceKind: option.resourceKind,
			catalogVersionId: option.id,
			requirementKind,
			displayOrder: order++
		});
	}
	return next;
}
