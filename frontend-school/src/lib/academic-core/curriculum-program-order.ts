export type OrderedCurriculumProgram = { id: string; code: string; displayOrder: number };
const textOrder = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
export function orderedCurriculumPrograms<T extends OrderedCurriculumProgram>(
	programs: readonly T[]
): T[] {
	return [...programs].sort(
		(a, b) => a.displayOrder - b.displayOrder || textOrder(a.code, b.code) || textOrder(a.id, b.id)
	);
}
export function moveCurriculumItem<T>(items: readonly T[], from: number, to: number): T[] {
	const result = [...items];
	if (
		!Number.isInteger(from) ||
		!Number.isInteger(to) ||
		from < 0 ||
		to < 0 ||
		from >= result.length ||
		to >= result.length ||
		from === to
	)
		return result;
	const [item] = result.splice(from, 1);
	result.splice(to, 0, item);
	return result;
}

export function orderedCurriculumProgramOptions<
	T extends OrderedCurriculumProgram & { curriculumLevelId: string }
>(programs: readonly T[]): T[] {
	const levels = [...new Set(programs.map((program) => program.curriculumLevelId))];
	return levels.flatMap((levelId) =>
		orderedCurriculumPrograms(programs.filter((program) => program.curriculumLevelId === levelId))
	);
}

export type CurriculumMutationBaselines = {
	requirementVersions: Record<string, number>;
	slotVersion: number;
};
// Only revisions attributable to this successful atomic order save may advance
// unrelated draft baselines. A refreshed metadata snapshot is never sufficient.
export function rebaseAfterOwnProgramOrder(
	baselines: CurriculumMutationBaselines,
	before: readonly { id: string; rowVersion: number }[],
	after: readonly { id: string; displayOrder: number; rowVersion: number }[],
	expectedLevelVersion: number,
	resultLevelVersion: number
): CurriculumMutationBaselines {
	const requirementVersions = { ...baselines.requirementVersions };
	for (const result of after) {
		const original = before.find((program) => program.id === result.id);
		const baseline = requirementVersions[result.id];
		if (!original || baseline !== original.rowVersion) continue;
		requirementVersions[result.id] = result.rowVersion;
	}
	return {
		requirementVersions,
		slotVersion:
			baselines.slotVersion === expectedLevelVersion ? resultLevelVersion : baselines.slotVersion
	};
}
