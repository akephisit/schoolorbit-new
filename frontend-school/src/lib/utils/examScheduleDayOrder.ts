export type ExamDayOrderInput = {
	id?: string | null;
	examDate: string;
	startTime?: string | null;
	sortOrder?: number | null;
};

function normalizedTime(value: string | null | undefined): string {
	return value?.slice(0, 5) ?? '';
}

export function compareExamDaysByDate(
	left: ExamDayOrderInput,
	right: ExamDayOrderInput
): number {
	const dateCompare = left.examDate.localeCompare(right.examDate);
	if (dateCompare !== 0) return dateCompare;

	const timeCompare = normalizedTime(left.startTime).localeCompare(normalizedTime(right.startTime));
	if (timeCompare !== 0) return timeCompare;

	const sortCompare = (left.sortOrder ?? 0) - (right.sortOrder ?? 0);
	if (sortCompare !== 0) return sortCompare;

	return String(left.id ?? '').localeCompare(String(right.id ?? ''));
}

export function nextSortOrderForDate(
	days: ExamDayOrderInput[],
	examDate: string,
	startTime: string,
	editingDayId: string | null
): number {
	const existingDay = editingDayId ? days.find((day) => day.id === editingDayId) : null;
	const candidateId = editingDayId ?? '__new_exam_day__';
	const candidate: ExamDayOrderInput = {
		id: candidateId,
		examDate,
		startTime,
		sortOrder: existingDay?.sortOrder ?? days.length + 1
	};
	const orderedDays = [
		...days.filter((day) => day.id !== editingDayId),
		candidate
	].sort(compareExamDaysByDate);
	const nextIndex = orderedDays.findIndex((day) => day.id === candidateId);

	return nextIndex === -1 ? days.length + 1 : nextIndex + 1;
}
