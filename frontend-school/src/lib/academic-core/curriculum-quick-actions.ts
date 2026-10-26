import type {
	CurriculumManagementOptions,
	CurriculumStructureRequirement,
	CurriculumStructureRequirementInput,
	CurriculumStructureWorkspace
} from '../api/academic-core.js';
import {
	appendSelectedCatalogRequirements,
	type CurriculumSelectionContext,
	type StagedCurriculumRequirement
} from './curriculum-catalog-selection.ts';

export type CurriculumQuickActionContext = CurriculumSelectionContext & {
	section?: CurriculumStructureRequirement['section'];
};
export type CurriculumQuickAction =
	| { kind: 'add-course' | 'add-activity'; context: CurriculumQuickActionContext }
	| { kind: 'edit' | 'remove'; context: CurriculumQuickActionContext; requirementId: string };
export type CurriculumQuickActionBaseline = {
	action: CurriculumQuickAction;
	levelId: string;
	draftId: string;
	rowVersion: number;
	programName: string;
	gradeName: string;
	termName: string;
	requirements: CurriculumStructureRequirement[];
	target?: CurriculumStructureRequirement;
};

export function stageCurriculumRequirements(
	requirements: readonly CurriculumStructureRequirement[]
): StagedCurriculumRequirement[] {
	return requirements.map((row) => ({
		studyProgramId: row.studyProgramId,
		gradeLevelId: row.gradeLevel.id,
		termSlotId: row.termSlotId,
		resourceKind: row.resourceKind,
		catalogVersionId: row.catalogVersionId,
		requirementKind: row.requirementKind,
		displayOrder: row.displayOrder
	}));
}

export function programRequirementInputs(
	requirements: readonly StagedCurriculumRequirement[],
	programId: string
): CurriculumStructureRequirementInput[] {
	return requirements
		.filter((row) => row.studyProgramId === programId)
		.map(({ studyProgramId: _programId, ...row }) => row);
}

export function captureCurriculumQuickAction(
	workspace: CurriculumStructureWorkspace,
	action: CurriculumQuickAction,
	canManage: boolean
): CurriculumQuickActionBaseline | null {
	const { studyProgramId, gradeLevelId, termSlotId } = action.context;
	const program = workspace.programs.find((row) => row.id === studyProgramId);
	const grade = workspace.gradeLevels.find((row) => row.id === gradeLevelId);
	const term = workspace.termSlots.find((row) => row.id === termSlotId);
	if (!canManage || !workspace.level.draftId || !program || !grade || !term) return null;
	const target =
		'requirementId' in action
			? workspace.requirements.find(
					(row) =>
						row.id === action.requirementId &&
						row.studyProgramId === studyProgramId &&
						row.gradeLevel.id === gradeLevelId &&
						row.termSlotId === termSlotId
				)
			: undefined;
	if ('requirementId' in action && !target) return null;
	const snapshotAction: CurriculumQuickAction = {
		...action,
		context: { studyProgramId, gradeLevelId, termSlotId, section: action.context.section }
	};
	return structuredClone({
		action: snapshotAction,
		levelId: workspace.level.id,
		draftId: workspace.level.draftId,
		rowVersion: program.rowVersion,
		programName: program.nameTh,
		gradeName: grade.name,
		termName: term.name,
		requirements: workspace.requirements.filter((row) => row.studyProgramId === studyProgramId),
		target
	});
}

export function quickActionRequirements(
	baseline: CurriculumQuickActionBaseline,
	change: {
		requirementKind: CurriculumStructureRequirementInput['requirementKind'];
		selectedIds?: string[];
		options?: CurriculumManagementOptions['catalogVersions'];
	}
): CurriculumStructureRequirementInput[] {
	const { action, requirements } = baseline;
	let staged = stageCurriculumRequirements(requirements);
	if (action.kind === 'edit' || action.kind === 'remove') {
		const index = requirements.findIndex((row) => row.id === action.requirementId);
		if (index < 0) throw new Error('ไม่พบรายการที่เลือกในร่างนี้');
		staged =
			action.kind === 'remove'
				? staged.filter((_, rowIndex) => rowIndex !== index)
				: staged.map((row, rowIndex) =>
						rowIndex === index ? { ...row, requirementKind: change.requirementKind } : row
					);
	} else {
		const kind = action.kind === 'add-course' ? 'course' : 'activity';
		staged = appendSelectedCatalogRequirements(
			staged,
			change.selectedIds ?? [],
			{
				studyProgramId: action.context.studyProgramId,
				gradeLevelId: action.context.gradeLevelId,
				termSlotId: action.context.termSlotId
			},
			(change.options ?? []).filter((option) => option.resourceKind === kind),
			requirements,
			change.requirementKind
		);
	}
	return programRequirementInputs(staged, action.context.studyProgramId);
}
