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
	'job_position_id' | 'academic_rank' | 'education_level' | 'major_id' | 'university_id'
>;
export function buildStaffPersonnelPatch(
	before: StaffPersonnelDraft,
	after: StaffPersonnelDraft
): Schemas['UpdateStaffInfoRequest'] | undefined {
	const patch: Schemas['UpdateStaffInfoRequest'] = {};
	if ((before.job_position_id ?? null) !== (after.job_position_id ?? null))
		patch.job_position_id = after.job_position_id ?? null;
	if ((before.academic_rank ?? null) !== (after.academic_rank ?? null))
		patch.academic_rank = after.academic_rank ?? null;
	if ((before.education_level ?? null) !== (after.education_level ?? null))
		patch.education_level = after.education_level ?? null;
	if ((before.major_id ?? null) !== (after.major_id ?? null))
		patch.major_id = after.major_id ?? null;
	if ((before.university_id ?? null) !== (after.university_id ?? null))
		patch.university_id = after.university_id ?? null;
	return Object.keys(patch).length ? patch : undefined;
}
export function staffPersonnelDraft(
	info: Schemas['StaffInfoResponse'] | null | undefined
): StaffPersonnelDraft {
	return {
		job_position_id: info?.job_position?.id ?? null,
		academic_rank: info?.academic_rank ?? null,
		education_level: info?.education_level ?? null,
		major_id: info?.major?.id ?? null,
		university_id: info?.university?.id ?? null
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
