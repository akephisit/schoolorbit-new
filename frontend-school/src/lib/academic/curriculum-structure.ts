import type {
	CurriculumDocumentSection,
	CurriculumStructureRequirement,
	CurriculumStructureWorkspace,
	StudyProgram
} from '$lib/api/academic-core';

const sectionDefinitions: ReadonlyArray<{
	id: CurriculumDocumentSection;
	label: string;
}> = [
	{ id: 'basic_course', label: 'รายวิชาพื้นฐาน' },
	{ id: 'additional_course', label: 'รายวิชาเพิ่มเติม' },
	{ id: 'student_development', label: 'กิจกรรมพัฒนาผู้เรียน' }
];

export type CurriculumDocumentSectionView = {
	id: CurriculumDocumentSection;
	label: string;
	rows: CurriculumStructureRequirement[];
	totalCredits: string;
	totalHours: string;
};

export type CurriculumTermPanel = {
	id: string;
	name: string;
	sections: CurriculumDocumentSectionView[];
	totalCredits: string;
	totalHours: string;
};

export type CurriculumDocumentView = {
	program: StudyProgram | null;
	gradeName: string;
	termPanels: CurriculumTermPanel[];
	totalCredits: string;
	totalHours: string;
};

export type ProgramComparisonCell = {
	termNames: string[];
	requirementKinds: CurriculumStructureRequirement['requirementKind'][];
	credit: string | null;
	totalHours: string | null;
};

export type ProgramComparisonRow = {
	key: string;
	catalogVersionId: string;
	resourceKind: CurriculumStructureRequirement['resourceKind'];
	section: CurriculumDocumentSection;
	code: string;
	name: string;
	cells: Record<string, ProgramComparisonCell | undefined>;
	isDifferent: boolean;
};

export type ProgramComparisonView = {
	programs: Array<{ id: string; name: string; code: string; isDefault: boolean }>;
	sections: typeof sectionDefinitions;
	rows: ProgramComparisonRow[];
};

function decimalToHundredths(value: string | null | undefined): number | null {
	if (value == null || value.trim() === '') return null;
	const match = /^(\d+)(?:\.(\d{1,2}))?$/.exec(value.trim());
	if (!match) return null;
	const fraction = (match[2] ?? '').padEnd(2, '0');
	return Number(match[1]) * 100 + Number(fraction || '0');
}

function formatHundredths(value: number): string {
	return `${Math.floor(value / 100)}.${String(value % 100).padStart(2, '0')}`;
}

function sumMetric(
	requirements: CurriculumStructureRequirement[],
	metric: 'credit' | 'totalHours'
): string {
	const total = requirements.reduce((sum, requirement) => {
		return sum + (decimalToHundredths(requirement.metrics[metric]) ?? 0);
	}, 0);
	return formatHundredths(total);
}

export function buildCurriculumDocument(
	workspace: CurriculumStructureWorkspace,
	studyProgramId: string,
	gradeLevelId: string
): CurriculumDocumentView {
	const program = workspace.programs.find((item) => item.id === studyProgramId) ?? null;
	const gradeName =
		workspace.gradeLevels.find((item) => item.id === gradeLevelId)?.name ?? 'ไม่พบระดับชั้น';
	const selected = workspace.requirements.filter(
		(requirement) =>
			requirement.studyProgramId === studyProgramId && requirement.gradeLevel.id === gradeLevelId
	);
	const termPanels = [...workspace.termSlots]
		.sort((left, right) => left.sequence - right.sequence)
		.map((slot) => {
			const termRequirements = selected.filter((requirement) => requirement.termSlotId === slot.id);
			const sections = sectionDefinitions.map((section) => {
				const rows = termRequirements
					.filter((requirement) => requirement.section === section.id)
					.sort(
						(left, right) =>
							left.displayOrder - right.displayOrder || left.code.localeCompare(right.code, 'th')
					);
				return {
					...section,
					rows,
					totalCredits: sumMetric(rows, 'credit'),
					totalHours: sumMetric(rows, 'totalHours')
				};
			});
			return {
				id: slot.id,
				name: slot.name,
				sections,
				totalCredits: sumMetric(termRequirements, 'credit'),
				totalHours: sumMetric(termRequirements, 'totalHours')
			};
		});

	return {
		program,
		gradeName,
		termPanels,
		totalCredits: sumMetric(selected, 'credit'),
		totalHours: sumMetric(selected, 'totalHours')
	};
}

export function buildProgramComparison(
	workspace: CurriculumStructureWorkspace,
	gradeLevelId: string
): ProgramComparisonView {
	const slotNames = new Map(workspace.termSlots.map((slot) => [slot.id, slot.name]));
	const rows = new Map<string, ProgramComparisonRow>();
	for (const requirement of workspace.requirements) {
		if (requirement.gradeLevel.id !== gradeLevelId) continue;
		const key = `${requirement.resourceKind}:${requirement.catalogVersionId}`;
		const row = rows.get(key) ?? {
			key,
			catalogVersionId: requirement.catalogVersionId,
			resourceKind: requirement.resourceKind,
			section: requirement.section,
			code: requirement.code,
			name: requirement.name,
			cells: {},
			isDifferent: false
		};
		const existing = row.cells[requirement.studyProgramId];
		row.cells[requirement.studyProgramId] = {
			termNames: [
				...(existing?.termNames ?? []),
				slotNames.get(requirement.termSlotId) ?? 'ไม่พบภาคเรียน'
			],
			requirementKinds: [...(existing?.requirementKinds ?? []), requirement.requirementKind],
			credit: requirement.metrics.credit ?? null,
			totalHours: requirement.metrics.totalHours ?? null
		};
		rows.set(key, row);
	}

	const programs = workspace.programs.map((program) => ({
		id: program.id,
		name: program.nameTh,
		code: program.code,
		isDefault: program.isDefault
	}));
	const orderedRows = [...rows.values()]
		.map((row) => {
			const signatures = programs.map((program) => JSON.stringify(row.cells[program.id] ?? null));
			return { ...row, isDifferent: new Set(signatures).size > 1 };
		})
		.sort((left, right) => {
			const sectionOrder =
				sectionDefinitions.findIndex((section) => section.id === left.section) -
				sectionDefinitions.findIndex((section) => section.id === right.section);
			return sectionOrder || left.code.localeCompare(right.code, 'th');
		});

	return { programs, sections: sectionDefinitions, rows: orderedRows };
}
