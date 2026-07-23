export type ExamDayOrderInput = {
	id?: string | null;
	examDate: string;
	startTime?: string | null;
};

function normalizedTime(value: string | null | undefined): string {
	return value?.slice(0, 5) ?? '';
}

export function compareExamDaysByDate(left: ExamDayOrderInput, right: ExamDayOrderInput): number {
	const dateCompare = left.examDate.localeCompare(right.examDate);
	if (dateCompare !== 0) return dateCompare;

	const timeCompare = normalizedTime(left.startTime).localeCompare(normalizedTime(right.startTime));
	if (timeCompare !== 0) return timeCompare;

	return String(left.id ?? '').localeCompare(String(right.id ?? ''));
}
