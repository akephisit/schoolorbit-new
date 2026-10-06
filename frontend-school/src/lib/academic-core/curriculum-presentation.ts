import type { StudyProgramOption } from '#lib/api/academic-core.js';
export function curriculumEditionLabel(edition: {
	editionName: string;
	revisionYear?: number | null;
}): string {
	const name = edition.editionName.trim();
	return edition.revisionYear && name === String(edition.revisionYear)
		? `ฉบับปรับปรุง พุทธศักราช ${edition.revisionYear}`
		: name;
}
export function studyProgramLabel(program: StudyProgramOption): string {
	return `${program.name} · ${curriculumEditionLabel(program)} · ${program.levelName}`;
}
export function programsForGrade(
	programs: StudyProgramOption[],
	gradeLevelId: string
): StudyProgramOption[] {
	return programs.filter((program) => program.gradeLevelIds.includes(gradeLevelId));
}
