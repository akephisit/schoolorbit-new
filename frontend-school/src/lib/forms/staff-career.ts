import type { components } from '$lib/api/generated/school-api';
type Schemas = components['schemas'];
export type StaffPersonnelType = Schemas['StaffPersonnelType'];
export type StaffCareerKind = Schemas['StaffCareerKind'];
export type StaffCareerEntry = Schemas['StaffCareerEntry'];
export type StaffCareerCurrent = Schemas['StaffCareerCurrent'];
export type StaffCareerCorrectionReasons = Partial<Record<StaffCareerKind, string>>;
export interface StaffCareerFactDraft<V> {
	value: V | null;
	effectiveDate: string;
	orderDate: string;
	orderNumber: string;
	note: string;
	reference: Schemas['StaffCareerReference'] | null;
}
export interface StaffCareerDraft {
	personnelType: StaffCareerFactDraft<StaffPersonnelType>;
	jobPosition: StaffCareerFactDraft<string>;
	academicRank: StaffCareerFactDraft<Schemas['StaffAcademicRank']>;
}
export const PERSONNEL_TYPE_LABELS = {
	civil_servant: 'ข้าราชการครู',
	government_employee: 'พนักงานราชการ',
	contract_employee: 'ลูกจ้างชั่วคราว / ครูอัตราจ้าง',
	permanent_employee: 'ลูกจ้างประจำ',
	other: 'อื่น ๆ'
} satisfies Record<StaffPersonnelType, string>;
export const CAREER_KIND_LABELS = {
	personnel_type: 'ประเภทบุคลากร',
	job_position: 'ตำแหน่ง',
	academic_rank: 'วิทยฐานะ'
} satisfies Record<StaffCareerKind, string>;
function details(entry: StaffCareerEntry | null | undefined) {
	return {
		effectiveDate: entry?.effectiveDate ?? '',
		orderDate: entry?.orderDate ?? '',
		orderNumber: entry?.orderNumber ?? '',
		note: entry?.note ?? '',
		reference: entry ? { id: entry.id, revision: entry.revision } : null
	};
}
export function staffCareerDraft(current: StaffCareerCurrent | null): StaffCareerDraft {
	return {
		personnelType: {
			value:
				current?.personnelType?.fact.kind === 'personnel_type'
					? current.personnelType.fact.value
					: null,
			...details(current?.personnelType)
		},
		jobPosition: {
			value:
				current?.jobPosition?.fact.kind === 'job_position' ? current.jobPosition.fact.value : null,
			...details(current?.jobPosition)
		},
		academicRank: {
			value:
				current?.academicRank?.fact.kind === 'academic_rank'
					? current.academicRank.fact.value
					: null,
			...details(current?.academicRank)
		}
	};
}
function text(value: string, limit: number, label: string): string | null {
	if (/\p{Cc}/u.test(value)) throw new Error(`${label}ต้องไม่มีอักขระควบคุม`);
	const normalized = value.trim();
	if (Array.from(normalized).length > limit)
		throw new Error(`${label}ต้องยาวไม่เกิน ${limit} ตัวอักษร`);
	return normalized || null;
}
function date(value: string): string | null {
	if (!value) return null;
	if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) throw new Error('วันที่ไม่ถูกต้อง');
	const parsed = new Date(`${value}T00:00:00Z`);
	if (!Number.isFinite(parsed.getTime()) || parsed.toISOString().slice(0, 10) !== value)
		throw new Error('วันที่ไม่ถูกต้อง');
	const today = new Intl.DateTimeFormat('sv-SE', {
		timeZone: 'Asia/Bangkok',
		year: 'numeric',
		month: '2-digit',
		day: '2-digit'
	}).format(new Date());
	if (value > today) throw new Error('วันที่ต้องไม่อยู่ในอนาคต');
	return value;
}
export function careerEntryInput(
	fact: Schemas['StaffCareerFact'],
	draft: StaffCareerFactDraft<string>
): Schemas['StaffCareerEntryInput'] {
	return {
		fact,
		effectiveDate: date(draft.effectiveDate),
		orderDate: date(draft.orderDate),
		orderNumber: text(draft.orderNumber, 100, 'เลขที่คำสั่ง'),
		note: text(draft.note, 1000, 'หมายเหตุ')
	};
}
function inputs(draft: StaffCareerDraft): Schemas['StaffCareerEntryInput'][] {
	return [
		careerEntryInput(
			{ kind: 'personnel_type', value: draft.personnelType.value },
			draft.personnelType
		),
		careerEntryInput({ kind: 'job_position', value: draft.jobPosition.value }, draft.jobPosition),
		careerEntryInput({ kind: 'academic_rank', value: draft.academicRank.value }, draft.academicRank)
	];
}
const keys = ['personnelType', 'jobPosition', 'academicRank'] as const;
export function buildCreateStaffCareer(
	draft: StaffCareerDraft
): Schemas['CreateStaffCareerRequest'] | undefined {
	const entries = inputs(draft).filter(
		(entry) =>
			entry.fact.value !== null ||
			entry.effectiveDate !== null ||
			entry.orderDate !== null ||
			entry.orderNumber !== null ||
			entry.note !== null
	);
	return entries.length ? { entries } : undefined;
}
export function buildUpdateStaffCareer(
	before: StaffCareerDraft,
	after: StaffCareerDraft,
	reasons: StaffCareerCorrectionReasons = {}
): Schemas['UpdateStaffCareerRequest'] | undefined {
	const previous = inputs(before),
		next = inputs(after);
	const changes: Schemas['StaffCareerCurrentChange'][] = [];
	next.forEach((entry, index) => {
		if (JSON.stringify(entry) === JSON.stringify(previous[index])) return;
		const key = keys[index],
			expectedCurrent = before[key].reference;
		const correction = entry.fact.value === previous[index].fact.value && expectedCurrent !== null;
		const reason = correction ? text(reasons[entry.fact.kind] ?? '', 1000, 'เหตุผล') : null;
		if (correction && !reason)
			throw new Error(`กรุณาระบุเหตุผลการแก้${CAREER_KIND_LABELS[entry.fact.kind]}`);
		changes.push({ expectedCurrent, entry, ...(reason ? { correctionReason: reason } : {}) });
	});
	return changes.length ? { changes } : undefined;
}
export function resetCareerDetailsForChangedValue(
	before: StaffCareerDraft,
	after: StaffCareerDraft
): StaffCareerDraft {
	const changed = structuredClone(after);
	for (const key of keys)
		if (before[key].value !== after[key].value)
			Object.assign(changed[key], { effectiveDate: '', orderDate: '', orderNumber: '', note: '' });
	return changed;
}
export function formatCareerDate(value: string | null): string {
	if (!value) return 'ยังไม่ระบุ';
	const parsed = new Date(`${value}T00:00:00Z`);
	if (
		!/^\d{4}-\d{2}-\d{2}$/.test(value) ||
		!Number.isFinite(parsed.getTime()) ||
		parsed.toISOString().slice(0, 10) !== value
	)
		return 'วันที่ไม่ถูกต้อง';
	return new Intl.DateTimeFormat('th-TH-u-ca-buddhist', {
		timeZone: 'UTC',
		day: 'numeric',
		month: 'short',
		year: 'numeric'
	}).format(parsed);
}

export function careerFieldErrors(draft: StaffCareerDraft): Record<string, string> {
	const errors: Record<string, string> = {};
	const kinds: StaffCareerKind[] = ['personnel_type', 'job_position', 'academic_rank'];
	keys.forEach((key, index) => {
		for (const field of ['effectiveDate', 'orderDate', 'orderNumber', 'note'] as const) {
			try {
				if (field === 'effectiveDate' || field === 'orderDate') date(draft[key][field]);
				else
					text(
						draft[key][field],
						field === 'orderNumber' ? 100 : 1000,
						field === 'orderNumber' ? 'เลขที่คำสั่ง' : 'หมายเหตุ'
					);
			} catch (error) {
				errors[`${kinds[index]}.${field}`] =
					error instanceof Error ? error.message : 'ข้อมูลไม่ถูกต้อง';
			}
		}
	});
	return errors;
}
export function needsCareerCorrection(
	before: StaffCareerFactDraft<string> | undefined,
	after: StaffCareerFactDraft<string>
): boolean {
	return Boolean(
		before?.reference &&
		before.value === after.value &&
		(['effectiveDate', 'orderDate', 'orderNumber', 'note'] as const).some(
			(field) => before[field] !== after[field]
		)
	);
}
