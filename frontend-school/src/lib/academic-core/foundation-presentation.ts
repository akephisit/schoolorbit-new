export const ACADEMIC_WEEKDAYS = ['MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT', 'SUN'] as const;

export type AcademicWeekday = (typeof ACADEMIC_WEEKDAYS)[number];
export type AcademicTermKind = 'regular' | 'summer' | 'remedial' | 'custom';

export function standardAcademicYearName(year: number): string {
	return `ปีการศึกษา ${year}`;
}

export function standardTermName(termType: AcademicTermKind, sequence: number): string {
	switch (termType) {
		case 'regular':
			return `ภาคเรียนที่ ${sequence}`;
		case 'summer':
			return 'ภาคฤดูร้อน';
		case 'remedial':
			return 'ภาคซ่อมเสริม';
		case 'custom':
			return `ภาคเรียนกำหนดเอง ${sequence}`;
	}
}

export function customNameFromStored(storedName: string, standardName: string): string {
	const normalizedStored = storedName.trim();
	const generatedPatterns = [
		/^ปีการศึกษา\s+\d+$/u,
		/^ภาคเรียนที่\s+\d+$/u,
		/^ภาคฤดูร้อน$/u,
		/^ภาคซ่อมเสริม$/u,
		/^ภาคเรียนกำหนดเอง\s+\d+$/u,
		/^(?:อ\.|ป\.|ม\.)\d+\/.+$/u
	];
	return normalizedStored === standardName.trim() ||
		generatedPatterns.some((pattern) => pattern.test(normalizedStored))
		? ''
		: normalizedStored;
}

export function normalizeSchoolDays(days: readonly string[]): AcademicWeekday[] {
	const normalized = new Set(days.map((day) => day.trim().toUpperCase()));
	return ACADEMIC_WEEKDAYS.filter((day) => normalized.has(day));
}
