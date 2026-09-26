export function readCurriculumAlignmentContext(url: URL) {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() ?? '';
	const academicTermId = url.searchParams.get('academicTermId')?.trim() ?? '';
	if (!academicYearId || !academicTermId) return null;
	return {
		academicYearId,
		academicTermId,
		studyProgramId: url.searchParams.get('studyProgramId')?.trim() || undefined,
		timetableVersionId: url.searchParams.get('timetableVersionId')?.trim() || undefined
	};
}

export function curriculumAlignmentContextKey(
	context: ReturnType<typeof readCurriculumAlignmentContext>
) {
	return context
		? `${context.academicYearId}:${context.academicTermId}:${context.studyProgramId ?? ''}:${context.timetableVersionId ?? ''}`
		: '';
}
