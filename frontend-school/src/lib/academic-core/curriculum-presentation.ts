import type {
	CurriculumOverviewItem,
	CurriculumVersion,
	StudyProgramOption
} from '#lib/api/academic-core.js';

export function curriculumEditionLabel(
	version: Pick<CurriculumVersion, 'versionName' | 'revisionYear'>
): string {
	const name = version.versionName.trim();
	if (version.revisionYear && name === String(version.revisionYear)) {
		return `ฉบับปรับปรุง พุทธศักราช ${version.revisionYear}`;
	}
	return name;
}

export function studyProgramLabel(program: StudyProgramOption): string {
	return `${program.name} · ${curriculumEditionLabel(program)} · ${program.curriculumName}`;
}

export function programsForGrade(
	programs: StudyProgramOption[],
	gradeLevelId: string
): StudyProgramOption[] {
	return programs.filter((program) => program.gradeLevelIds.includes(gradeLevelId));
}

export function groupCurriculaByRevision(items: CurriculumOverviewItem[]) {
	const groups = new Map<string, CurriculumOverviewItem[]>();
	for (const item of items) {
		const label = item.displayVersion
			? curriculumEditionLabel(item.displayVersion)
			: 'ยังไม่มีฉบับที่เผยแพร่';
		const group = groups.get(label) ?? [];
		group.push(item);
		groups.set(label, group);
	}
	return Array.from(groups, ([label, curricula]) => ({ label, items: curricula }));
}
