import type { CreateStaffRequest, StaffProfileResponse } from '#lib/api/staff.js';

export const STAFF_NAME_FIELDS = [
	'title',
	'first_name',
	'last_name',
	'title_en',
	'first_name_en',
	'last_name_en'
] as const;
export type StaffNameField = (typeof STAFF_NAME_FIELDS)[number];
export type StaffNameDraft = Record<StaffNameField, string>;
export function staffNameDraft(
	value?: Pick<StaffProfileResponse, StaffNameField> | null
): StaffNameDraft {
	return Object.fromEntries(
		STAFF_NAME_FIELDS.map((field) => [field, value?.[field] ?? ''])
	) as StaffNameDraft;
}
export function completeStaffName(value: StaffNameDraft, language: 'th' | 'en'): boolean {
	const fields = language === 'th' ? STAFF_NAME_FIELDS.slice(0, 3) : STAFF_NAME_FIELDS.slice(3);
	return fields.every((field) => value[field].trim().length > 0);
}
export function preferredStaffName(value: StaffNameDraft): string {
	if (!completeStaffName(value, 'th') && completeStaffName(value, 'en'))
		return [value.title_en, value.first_name_en, value.last_name_en]
			.map((part) => part.trim())
			.join(' ');
	return [value.title.trim() + value.first_name.trim(), value.last_name.trim()]
		.filter(Boolean)
		.join(' ');
}
export function staffNameErrors(value: StaffNameDraft): Record<string, string> {
	const errors: Record<string, string> = {};
	for (const field of STAFF_NAME_FIELDS) {
		if (/\p{Cc}/u.test(value[field])) errors[field] = 'ต้องไม่มีอักขระควบคุม';
		else if (Array.from(value[field].trim()).length > (field.startsWith('title') ? 50 : 100))
			errors[field] = `ยาวไม่เกิน ${field.startsWith('title') ? 50 : 100} ตัวอักษร`;
	}
	if (!completeStaffName(value, 'th') && !completeStaffName(value, 'en'))
		errors.names = 'กรอกคำนำหน้า ชื่อ และนามสกุลให้ครบอย่างน้อยหนึ่งภาษา';
	return errors;
}
export function staffNameInput(value: StaffNameDraft): Pick<CreateStaffRequest, StaffNameField> {
	return Object.fromEntries(STAFF_NAME_FIELDS.map((field) => [field, value[field].trim()])) as Pick<
		CreateStaffRequest,
		StaffNameField
	>;
}
