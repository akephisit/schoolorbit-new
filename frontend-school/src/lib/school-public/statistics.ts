import type { PublicSchoolStatistics } from '#lib/api/school.js';

type Counts = PublicSchoolStatistics['students'];
export interface PublicLevelSummary {
	id: string;
	label: string;
	students: Counts;
}

export function summarizeEducationLevels(statistics: PublicSchoolStatistics): PublicLevelSummary[] {
	const groups = new Map<string, PublicLevelSummary>();
	for (const grade of statistics.grades) {
		const [id, label] =
			grade.levelType === 'kindergarten'
				? ['0-kindergarten', 'อนุบาล']
				: grade.levelType === 'primary'
					? ['1-primary', 'ประถมศึกษา']
					: grade.levelType === 'secondary' && grade.year >= 1 && grade.year <= 3
						? ['2-secondary-lower', 'มัธยมศึกษาตอนต้น']
						: grade.levelType === 'secondary' && grade.year >= 4 && grade.year <= 6
							? ['3-secondary-upper', 'มัธยมศึกษาตอนปลาย']
							: [
									`4-${grade.levelType}`,
									grade.levelType === 'secondary' ? 'มัธยมศึกษาอื่น ๆ' : grade.levelType
								];
		let group = groups.get(id);
		if (!group) {
			group = { id, label, students: { male: 0, female: 0, otherOrUnspecified: 0, total: 0 } };
			groups.set(id, group);
		}
		for (const key of ['male', 'female', 'otherOrUnspecified', 'total'] as const)
			group.students[key] += grade.students[key];
	}
	return [...groups.values()].sort((a, b) => a.id.localeCompare(b.id));
}
