import type { CreateStudentRequest, UpdateStudentRequest } from '#lib/api/students.js';
import type { components } from '#lib/api/generated/school-api.js';

export type StudentRegisterMode = 'create' | 'edit';

export function studentRegisterReadScope(access: {
	school: boolean;
	assigned: boolean;
	own: boolean;
	piiSchool: boolean;
	piiAssigned: boolean;
	piiOwn: boolean;
}): string {
	return [
		access.school,
		access.assigned,
		access.own,
		access.piiSchool,
		access.piiAssigned,
		access.piiOwn
	]
		.map((allowed) => (allowed ? '1' : '0'))
		.join('');
}

export const REGISTRATION_STATUS_LABELS = {
	unrecorded: 'ยังไม่ระบุ',
	pending: 'รอเข้าเรียน',
	studying: 'กำลังศึกษา',
	transferred_out: 'ย้ายออก',
	withdrawn: 'ลาออก/พ้นสภาพ',
	graduated: 'สำเร็จการศึกษา'
} as const satisfies Record<components['schemas']['StudentRegistrationStatus'], string>;
export type StudentRegistrationStatus = components['schemas']['StudentRegistrationStatus'];

export const STUDENT_ACCOUNT_STATUS_LABELS = {
	active: 'ใช้งาน',
	inactive: 'ไม่ใช้งาน',
	suspended: 'ระงับบัญชี',
	resigned: 'resigned',
	retired: 'retired'
} as const;

export function studentAccountStatus(value: string | null): string | undefined {
	return value && Object.hasOwn(STUDENT_ACCOUNT_STATUS_LABELS, value) ? value : undefined;
}

export function registrationStatus(value: string | null): StudentRegistrationStatus | undefined {
	return value && Object.hasOwn(REGISTRATION_STATUS_LABELS, value)
		? (value as StudentRegistrationStatus)
		: undefined;
}

export function accountStatusLabel(value: string | null | undefined): string {
	return value
		? (STUDENT_ACCOUNT_STATUS_LABELS[value as keyof typeof STUDENT_ACCOUNT_STATUS_LABELS] ?? value)
		: '—';
}

export interface StudentParentDraft {
	title: string;
	first_name: string;
	last_name: string;
	phone: string;
	relationship: string;
	national_id: string;
	email: string;
}

export interface StudentRegisterDraft {
	student_id: string;
	enrollment_date: string;
	registration_status: StudentRegistrationStatus;
	title: string;
	first_name: string;
	last_name: string;
	date_of_birth: string;
	gender: string;
	email: string;
	phone: string;
	address: string;
	national_id: string;
	password: string;
	confirmPassword: string;
	parentsEnabled: boolean;
	parents: StudentParentDraft[];
}

type StudentRegisterSeed = Partial<
	Record<
		| 'student_id'
		| 'enrollment_date'
		| 'registration_status'
		| 'title'
		| 'first_name'
		| 'last_name'
		| 'date_of_birth'
		| 'gender'
		| 'email'
		| 'phone'
		| 'address',
		string | null
	>
>;

export function studentRegisterDraft(value?: StudentRegisterSeed | null): StudentRegisterDraft {
	return {
		student_id: value?.student_id ?? '',
		enrollment_date: value?.enrollment_date ?? '',
		registration_status:
			registrationStatus(value?.registration_status ?? null) ?? (value ? 'unrecorded' : 'pending'),
		title: value?.title ?? '',
		first_name: value?.first_name ?? '',
		last_name: value?.last_name ?? '',
		date_of_birth: value?.date_of_birth ?? '',
		gender: value?.gender ?? '',
		email: value?.email ?? '',
		phone: value?.phone ?? '',
		address: value?.address ?? '',
		national_id: '',
		password: '',
		confirmPassword: '',
		parentsEnabled: false,
		parents: []
	};
}

export function studentParentDraft(): StudentParentDraft {
	return {
		title: '',
		first_name: '',
		last_name: '',
		phone: '',
		relationship: 'ผู้ปกครอง',
		national_id: '',
		email: ''
	};
}

function validEmail(value: string): boolean {
	return /^[^\s@]+@[^\s@]+\.[^\s@]+$/u.test(value);
}

function validDate(value: string): boolean {
	if (!/^\d{4}-\d{2}-\d{2}$/u.test(value)) return false;
	const parsed = new Date(`${value}T00:00:00Z`);
	return Number.isFinite(parsed.getTime()) && parsed.toISOString().slice(0, 10) === value;
}

export function studentPasswordError(password: string): string | null {
	const length = Array.from(password).length;
	if (length < 8 || length > 128) return 'รหัสผ่านต้องมี 8–128 ตัวอักษร';
	if (new TextEncoder().encode(password).length > 71) return 'รหัสผ่านต้องไม่เกิน 71 ไบต์';
	return null;
}

export function studentParentErrors(value: StudentParentDraft): Record<string, string> {
	const errors: Record<string, string> = {};
	if (!value.first_name.trim()) errors.first_name = 'กรุณากรอกชื่อ';
	if (!value.last_name.trim()) errors.last_name = 'กรุณากรอกนามสกุล';
	if (!/^\d{9,10}$/u.test(value.phone.trim())) errors.phone = 'กรุณากรอกเบอร์โทร 9–10 หลัก';
	if (!value.relationship.trim()) errors.relationship = 'กรุณาระบุความสัมพันธ์';
	if (value.email.trim() && !validEmail(value.email.trim())) errors.email = 'รูปแบบอีเมลไม่ถูกต้อง';
	if (value.national_id.trim() && !/^\d{13}$/u.test(value.national_id.trim()))
		errors.national_id = 'เลขบัตรประชาชนต้องเป็นตัวเลข 13 หลัก';
	for (const field of ['first_name', 'last_name', 'relationship', 'title', 'email'] as const) {
		const limit =
			field === 'title' || field === 'relationship' ? 50 : field === 'email' ? 255 : 100;
		if (Array.from(value[field].trim()).length > limit)
			errors[field] = `ระบุไม่เกิน ${limit} ตัวอักษร`;
	}
	return errors;
}

export function studentRegisterErrors(
	value: StudentRegisterDraft,
	mode: StudentRegisterMode
): Record<string, string> {
	const errors: Record<string, string> = {};
	if (!value.first_name.trim()) errors.first_name = 'กรุณากรอกชื่อ';
	if (!value.last_name.trim()) errors.last_name = 'กรุณากรอกนามสกุล';
	if (value.email.trim() && !validEmail(value.email.trim())) errors.email = 'รูปแบบอีเมลไม่ถูกต้อง';
	if (value.date_of_birth && !validDate(value.date_of_birth))
		errors.date_of_birth = 'กรุณาเลือกวันเกิดที่ถูกต้อง';
	if (value.enrollment_date && !validDate(value.enrollment_date))
		errors.enrollment_date = 'กรุณาเลือกวันที่เข้าเรียนที่ถูกต้อง';
	for (const [field, limit] of [
		['first_name', 100],
		['last_name', 100],
		['title', 50],
		['email', 255],
		['phone', 20],
		['address', 2000]
	] as const) {
		if (Array.from(value[field].trim()).length > limit)
			errors[field] = `ระบุไม่เกิน ${limit} ตัวอักษร`;
	}
	if (value.gender && !['male', 'female', 'other'].includes(value.gender))
		errors.gender = 'กรุณาเลือกเพศที่ระบบรองรับ';
	if (mode === 'create') {
		if (!value.student_id.trim()) errors.student_id = 'กรุณากรอกรหัสนักเรียน';
		else if (Array.from(value.student_id.trim()).length > 50)
			errors.student_id = 'ระบุไม่เกิน 50 ตัวอักษร';
		const passwordError = studentPasswordError(value.password);
		if (passwordError) errors.password = passwordError;
		if (value.password !== value.confirmPassword) errors.confirmPassword = 'รหัสผ่านไม่ตรงกัน';
		if (value.national_id.trim() && !/^\d{13}$/u.test(value.national_id.trim()))
			errors.national_id = 'เลขบัตรประชาชนต้องเป็นตัวเลข 13 หลัก';
		if (value.parentsEnabled) {
			if (!value.parents.length) errors.parents = 'กรุณาเพิ่มผู้ปกครองหรือปิดการเพิ่มผู้ปกครอง';
			if (value.parents.length > 10) errors.parents = 'เพิ่มผู้ปกครองได้ไม่เกิน 10 คน';
			for (const [index, parent] of value.parents.entries()) {
				for (const [field, error] of Object.entries(studentParentErrors(parent)))
					errors[`parents.${index}.${field}`] = error;
			}
		}
	}
	return errors;
}

/** Only enabled create-time guardians are submitted; hidden drafts stay local. */
export function studentRegisterParents(value: StudentRegisterDraft): StudentParentDraft[] {
	if (!value.parentsEnabled) return [];
	return value.parents.map((parent) => ({
		title: parent.title.trim(),
		first_name: parent.first_name.trim(),
		last_name: parent.last_name.trim(),
		phone: parent.phone.trim(),
		relationship: parent.relationship.trim(),
		national_id: parent.national_id.trim(),
		email: parent.email.trim()
	}));
}

export function createStudentRegisterInput(
	value: StudentRegisterDraft,
	allowNationalId: boolean
): CreateStudentRequest {
	return {
		student_id: value.student_id.trim(),
		registration_status: value.registration_status,
		enrollment_date: value.enrollment_date || undefined,
		title: value.title.trim() || undefined,
		first_name: value.first_name.trim(),
		last_name: value.last_name.trim(),
		date_of_birth: value.date_of_birth || undefined,
		gender: value.gender || undefined,
		email: value.email.trim() || undefined,
		phone: value.phone.trim() || undefined,
		address: value.address.trim() || undefined,
		national_id: allowNationalId ? value.national_id.trim() || undefined : undefined,
		password: value.password,
		parents: value.parentsEnabled
			? studentRegisterParents(value).map((parent) => ({
					...parent,
					title: parent.title || undefined,
					email: parent.email || undefined,
					national_id: allowNationalId ? parent.national_id || undefined : undefined
				}))
			: undefined
	};
}

export function updateStudentRegisterInput(value: StudentRegisterDraft): UpdateStudentRequest {
	return {
		first_name: value.first_name.trim(),
		last_name: value.last_name.trim(),
		title: value.title.trim() || null,
		date_of_birth: value.date_of_birth || null,
		gender: value.gender || null,
		email: value.email.trim() || null,
		phone: value.phone.trim() || null,
		address: value.address.trim() || null,
		registration_status: value.registration_status,
		enrollment_date: value.enrollment_date || null
	};
}
