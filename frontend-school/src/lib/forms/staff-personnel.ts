import {
	staffCareerDraft,
	PERSONNEL_TYPE_LABELS,
	buildUpdateStaffCareer,
	type StaffCareerDraft,
	type StaffCareerCorrectionReasons
} from './staff-career.ts';
import type { components } from '$lib/api/generated/school-api';
type Schemas = components['schemas'];
export const ACADEMIC_RANK_LABELS = {
	none: 'ไม่มีวิทยฐานะ',
	not_applicable: 'ไม่ใช้กับตำแหน่งนี้',
	proficient: 'ชำนาญการ',
	senior_proficient: 'ชำนาญการพิเศษ',
	expert: 'เชี่ยวชาญ',
	senior_expert: 'เชี่ยวชาญพิเศษ'
} satisfies Record<Schemas['StaffAcademicRank'], string>;
export const EDUCATION_LEVEL_LABELS = {
	primary: 'ประถมศึกษา',
	lower_secondary: 'มัธยมศึกษาตอนต้น',
	upper_secondary: 'มัธยมศึกษาตอนปลาย',
	vocational_certificate: 'ปวช.',
	higher_vocational: 'ปวส.',
	diploma: 'อนุปริญญา',
	bachelor: 'ปริญญาตรี',
	master: 'ปริญญาโท',
	doctorate: 'ปริญญาเอก',
	other: 'อื่น ๆ'
} satisfies Record<Schemas['StaffEducationLevel'], string>;
export type StaffPersonnelDraft = Pick<
	Schemas['UpdateStaffInfoRequest'],
	'education_level' | 'major' | 'university'
> & { career: StaffCareerDraft };
export function normalizeStaffEducationText(value: string | null | undefined): string | null {
	if (value == null) return null;
	if (/\p{Cc}/u.test(value)) throw new Error('ต้องไม่มีอักขระควบคุม');
	const normalized = value.trim();
	if (Array.from(normalized).length > 200) throw new Error('ต้องยาวไม่เกิน 200 ตัวอักษร');
	return normalized || null;
}
export function buildStaffPersonnelPatch(
	before: StaffPersonnelDraft,
	after: StaffPersonnelDraft,
	reasons: StaffCareerCorrectionReasons = {}
): Schemas['UpdateStaffInfoRequest'] | undefined {
	const patch: Schemas['UpdateStaffInfoRequest'] = {};
	const career = buildUpdateStaffCareer(before.career, after.career, reasons);
	if (career) patch.career = career;
	if ((before.education_level ?? null) !== (after.education_level ?? null))
		patch.education_level = after.education_level ?? null;
	if (normalizeStaffEducationText(before.major) !== normalizeStaffEducationText(after.major))
		patch.major = normalizeStaffEducationText(after.major);
	if (
		normalizeStaffEducationText(before.university) !== normalizeStaffEducationText(after.university)
	)
		patch.university = normalizeStaffEducationText(after.university);
	return Object.keys(patch).length ? patch : undefined;
}
export function staffPersonnelDraft(
	info: Schemas['StaffInfoResponse'] | null | undefined
): StaffPersonnelDraft {
	return {
		career: staffCareerDraft(info?.current_career ?? null),
		education_level: info?.education_level ?? null,
		major: info?.major ?? null,
		university: info?.university ?? null
	};
}
export function personnelDrilldownHref(
	dimension: Schemas['PersonnelDimension'],
	bucket: Schemas['PersonnelBucket'],
	status: Schemas['PersonnelStatusFilter']
): string {
	const parameters = {
		status: 'status',
		subject_group: 'subject_group_id',
		job_position: 'job_position_id',
		academic_rank: 'academic_rank',
		education_level: 'education_level'
	} as const;
	const query = new URLSearchParams({ status: dimension === 'status' ? bucket.key : status });
	if (dimension !== 'status') query.set(parameters[dimension], bucket.key);
	return `/staff/manage?${query}`;
}

export function staffCareerEntryLabel(entry: import('./staff-career.ts').StaffCareerEntry): string {
	const fact = entry.fact;
	if (fact.value === null) return 'ยังไม่ระบุ';
	if (fact.kind === 'academic_rank') return ACADEMIC_RANK_LABELS[fact.value];
	if (fact.kind === 'personnel_type') return PERSONNEL_TYPE_LABELS[fact.value];
	return entry.jobPosition?.name ?? 'ตำแหน่งที่ยังไม่มีรายละเอียด';
}
