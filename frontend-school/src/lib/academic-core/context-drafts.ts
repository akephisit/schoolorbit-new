import type { AcademicTerm, AcademicYear } from '#lib/api/academic-core.js';

export function yearDraftBase(year: AcademicYear) {
	return {
		id: year.id,
		year: year.year,
		name: year.name,
		schoolDays: year.schoolDays.toSorted()
	};
}

export function termDraftBase(term: AcademicTerm) {
	return {
		id: term.id,
		academicYearId: term.academicYearId,
		sequence: term.sequence,
		code: term.code,
		name: term.name,
		termType: term.termType,
		bellScheduleId: term.bellScheduleId,
		includedInYearResult: term.includedInYearResult,
		blocksYearClosure: term.blocksYearClosure
	};
}

export function yearDraftMetadataMatches(
	base: ReturnType<typeof yearDraftBase>,
	current: AcademicYear
): boolean {
	return JSON.stringify(base) === JSON.stringify(yearDraftBase(current));
}

export function termDraftMetadataMatches(
	base: ReturnType<typeof termDraftBase>,
	current: AcademicTerm
): boolean {
	return JSON.stringify(base) === JSON.stringify(termDraftBase(current));
}
