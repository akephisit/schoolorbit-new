import {
	staffCareerDraft,
	PERSONNEL_TYPE_LABELS,
	buildUpdateStaffCareer,
	type StaffCareerDraft,
	type StaffCareerCorrectionReasons
} from './staff-career.ts';
import type { components } from '#lib/api/generated/school-api.js';
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
export type StaffEducationDraft = {
	key: string;
	id: string | null;
	education_level: Schemas['StaffEducationLevel'] | null;
	major: string | null;
	university: string | null;
};
export type StaffPersonnelDraft = {
	career: StaffCareerDraft;
	educations: StaffEducationDraft[];
	education_revision: number;
	teaching_license_file_id: string | null;
	teaching_license_number: string;
	teaching_license_expiry: string;
};
export function newStaffEducation(): StaffEducationDraft {
	return { key: crypto.randomUUID(), id: null, education_level: null, major: '', university: '' };
}
export function staffEducationInput(row: StaffEducationDraft): Schemas['StaffEducationInput'] {
	return {
		...(row.id ? { id: row.id } : {}),
		education_level: row.education_level,
		major: normalizeStaffEducationText(row.major),
		university: normalizeStaffEducationText(row.university)
	};
}
export const EDUCATION_TIERS: Record<Schemas['StaffEducationLevel'], number> = {
	primary: 1,
	lower_secondary: 2,
	upper_secondary: 3,
	vocational_certificate: 3,
	higher_vocational: 4,
	diploma: 4,
	bachelor: 5,
	master: 6,
	doctorate: 7,
	other: 99
};
export function orderedStaffEducations<
	T extends { education_level: Schemas['StaffEducationLevel'] | null }
>(rows: readonly T[]): T[] {
	return rows.toSorted(
		(a, b) =>
			(a.education_level ? EDUCATION_TIERS[a.education_level] : 99) -
			(b.education_level ? EDUCATION_TIERS[b.education_level] : 99)
	);
}
export function staffEducationErrors(rows: StaffEducationDraft[]): Record<string, string> {
	const errors: Record<string, string> = {};
	for (const row of rows) {
		if (!row.id && !row.education_level)
			errors[`education.${row.key}.education_level`] = 'กรุณาระบุระดับวุฒิ';
		for (const field of ['major', 'university'] as const) {
			try {
				normalizeStaffEducationText(row[field]);
			} catch (error) {
				errors[`education.${row.key}.${field}`] =
					error instanceof Error ? error.message : 'ข้อมูลไม่ถูกต้อง';
			}
		}
	}
	return errors;
}
export function staffPersonnelErrors(value: StaffPersonnelDraft): Record<string, string> {
	const errors = staffEducationErrors(value.educations);
	if (
		/\p{Cc}/u.test(value.teaching_license_number) ||
		Array.from(value.teaching_license_number.trim()).length > 100
	)
		errors.teaching_license_number = 'เลขใบอนุญาตต้องยาวไม่เกิน 100 ตัวอักษรและไม่มีอักขระควบคุม';
	return errors;
}
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
	for (const field of ['teaching_license_number', 'teaching_license_expiry'] as const) {
		if (before[field] !== after[field]) patch[field] = after[field]?.trim() || null;
	}
	if (before.teaching_license_file_id !== after.teaching_license_file_id)
		patch.teaching_license_file = {
			expected_file_id: before.teaching_license_file_id,
			file_id: after.teaching_license_file_id
		};
	const oldRows = before.educations.map(staffEducationInput);
	const newRows = after.educations.map(staffEducationInput);
	if (JSON.stringify(oldRows) !== JSON.stringify(newRows))
		patch.education = { expected_revision: before.education_revision, items: newRows };
	return Object.keys(patch).length ? patch : undefined;
}
export function staffPersonnelDraft(
	info: Schemas['StaffInfoResponse'] | null | undefined
): StaffPersonnelDraft {
	return {
		career: staffCareerDraft(info?.current_career ?? null),
		education_revision: info?.education_revision ?? 0,
		teaching_license_file_id: info?.teaching_license_file_id ?? null,
		teaching_license_number: info?.teaching_license_number ?? '',
		teaching_license_expiry: info?.teaching_license_expiry ?? '',
		educations: (info?.educations ?? []).map((row) => ({ ...row, key: row.id }))
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
