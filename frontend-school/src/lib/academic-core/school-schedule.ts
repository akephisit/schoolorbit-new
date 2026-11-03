import type {
	AcademicTerm,
	AcademicYear,
	BellSchedulePeriod,
	BellSchedulePeriodsSnapshot
} from '#lib/api/academic-core.js';
import { normalizeSchoolDays, type AcademicWeekday } from './foundation-presentation.ts';

export const SCHOOL_WEEKDAYS: Array<{ code: AcademicWeekday; label: string }> = [
	{ code: 'MON', label: 'จันทร์' },
	{ code: 'TUE', label: 'อังคาร' },
	{ code: 'WED', label: 'พุธ' },
	{ code: 'THU', label: 'พฤหัสบดี' },
	{ code: 'FRI', label: 'ศุกร์' },
	{ code: 'SAT', label: 'เสาร์' },
	{ code: 'SUN', label: 'อาทิตย์' }
];

export type SchoolScheduleSource = {
	term: AcademicTerm;
	snapshot: BellSchedulePeriodsSnapshot;
	schoolDays: AcademicWeekday[];
};

export function schoolScheduleSourceFields(source: SchoolScheduleSource) {
	return {
		sourceTermId: source.term.id,
		sourceTermRowVersion: source.term.rowVersion,
		sourceBellScheduleRowVersion: source.snapshot.rowVersion,
		schoolDays: [...source.schoolDays]
	};
}

export function previousSchoolScheduleTerm(
	year: AcademicYear,
	years: readonly AcademicYear[],
	terms: readonly AcademicTerm[],
	target?: AcademicTerm
): AcademicTerm | undefined {
	const yearNumbers = new Map(years.map((item) => [item.id, item.year]));
	return terms
		.filter((term) => {
			const number = yearNumbers.get(term.academicYearId);
			return (
				number !== undefined &&
				(number < year.year ||
					(number === year.year && term.sequence < (target?.sequence ?? Infinity)))
			);
		})
		.toSorted(
			(a, b) =>
				yearNumbers.get(b.academicYearId)! - yearNumbers.get(a.academicYearId)! ||
				b.sequence - a.sequence
		)[0];
}

export type StoredPeriodSchoolDays =
	{ ok: true; days: AcademicWeekday[] | null } | { ok: false; error: string };

export function parsePeriodSchoolDays(
	period: Pick<BellSchedulePeriod, 'applicableDays'>
): StoredPeriodSchoolDays {
	if (period.applicableDays === null) return { ok: true, days: null };
	if (period.applicableDays === undefined)
		return { ok: false, error: 'ไม่พบข้อมูลวันเฉพาะคาบ กรุณาโหลดข้อมูลที่ครบถ้วนก่อนบันทึก' };
	const values = period.applicableDays.split(',');
	const normalized = normalizeSchoolDays(values);
	if (
		values.length === 0 ||
		normalized.length !== values.length ||
		values.some((day) => !SCHOOL_WEEKDAYS.some((item) => item.code === day))
	)
		return {
			ok: false,
			error: 'รูปแบบวันเฉพาะคาบที่บันทึกไว้ไม่ถูกต้อง กรุณาให้ผู้ดูแลตรวจข้อมูลเดิมก่อนบันทึก'
		};
	return { ok: true, days: normalized };
}

export function periodSchoolDayOverride(period: BellSchedulePeriod): AcademicWeekday[] | null {
	const parsed = parsePeriodSchoolDays(period);
	if (!parsed.ok) throw new Error(parsed.error);
	return parsed.days;
}

export function periodDaysOutsideTerm(
	periods: readonly Pick<BellSchedulePeriod, 'applicableDays'>[],
	schoolDays: readonly string[]
): AcademicWeekday[] {
	const selected = new Set(schoolDays);
	return normalizeSchoolDays(
		periods.flatMap((period) => {
			const parsed = parsePeriodSchoolDays(period);
			return parsed.ok ? (parsed.days ?? []) : [];
		})
	).filter((day) => !selected.has(day));
}

export function periodAllowsSchoolDay(
	period: Pick<BellSchedulePeriod, 'applicableDays' | 'isActive'> | undefined,
	day: string,
	schoolDays: readonly string[]
): boolean {
	if (!period?.isActive || !schoolDays.includes(day)) return false;
	const parsed = parsePeriodSchoolDays(period);
	return (
		parsed.ok &&
		(parsed.days === null ||
			(parsed.days.every((value) => schoolDays.includes(value)) &&
				parsed.days.includes(day as AcademicWeekday)))
	);
}
