import type {
	LeavePeriodInput,
	LeaveRequestDetail,
	LeavePolicyVersion,
	LeaveUnit,
	LeavePolicyUnit,
	LeaveKind,
	ProcessTemplateStep
} from '#lib/api/staff-leave.js';
import type { StaffPersonnelType } from '#lib/forms/staff-career.js';

export interface LeaveDraft {
	typeId: string;
	academicYearId: string;
	academicTermId: string;
	startDate?: string;
	endDate?: string;
	startTime: string;
	endTime: string;
	reason: string;
	evidenceFileIds: string[];
}
export function leaveDraft(detail?: LeaveRequestDetail): LeaveDraft {
	const source = detail?.request;
	return {
		typeId: source?.typeId ?? '',
		academicYearId: source?.academicYearId ?? '',
		academicTermId: source?.academicTermId ?? '',
		startDate: source?.startDate,
		endDate: source?.endDate,
		startTime: source?.startTime?.slice(0, 5) ?? '',
		endTime: source?.endTime?.slice(0, 5) ?? '',
		reason: source?.reason ?? '',
		evidenceFileIds: [...(detail?.evidenceFileIds ?? [])]
	};
}
export function leaveDraftErrors(
	draft: LeaveDraft,
	policy: LeavePolicyVersion | null
): Partial<Record<keyof LeaveDraft, string>> {
	const errors: Partial<Record<keyof LeaveDraft, string>> = {};
	if (!draft.typeId) errors.typeId = 'เลือกประเภทคำขอ';
	if (!draft.startDate) errors.startDate = 'เลือกวันที่เริ่ม';
	if (!draft.endDate) errors.endDate = 'เลือกวันที่สิ้นสุด';
	if (draft.startDate && draft.endDate && draft.endDate < draft.startDate)
		errors.endDate = 'วันที่สิ้นสุดต้องไม่น้อยกว่าวันที่เริ่ม';
	if (!draft.reason.trim()) errors.reason = 'ระบุเหตุผล';
	if (policy?.unit === 'minute') {
		if (!draft.startTime) errors.startTime = 'ระบุเวลาเริ่ม';
		if (!draft.endTime || draft.endTime <= draft.startTime)
			errors.endTime = 'เวลาสิ้นสุดต้องอยู่หลังเวลาเริ่ม';
		if (draft.endDate !== draft.startDate) errors.endDate = 'คำขอแบบนาทีต้องอยู่ในวันเดียวกัน';
	}

	return errors;
}
export function leavePeriodInput(draft: LeaveDraft, unit: LeavePolicyUnit): LeavePeriodInput {
	return {
		typeId: draft.typeId,
		academicYearId: draft.academicYearId || null,
		academicTermId: draft.academicTermId || null,
		startDate: draft.startDate ?? '',
		endDate: draft.endDate ?? '',
		startTime: unit === 'minute' ? `${draft.startTime}:00` : null,
		endTime: unit === 'minute' ? `${draft.endTime}:00` : null,
		reason: draft.reason.trim(),
		evidenceFileIds: draft.evidenceFileIds
	};
}
export function displayedPolicyLimit(amount: number | null | undefined, unit: LeaveUnit): string {
	return amount == null ? '' : String(unit === 'half_day' ? amount / 2 : amount);
}
export function policyLimitInput(raw: string): number | null {
	if (!raw.trim()) return null;
	const value = Number(raw);
	if (!Number.isSafeInteger(value) || value < 1)
		throw new Error('จำนวนต้องเป็นค่าบวกตามหน่วยที่เลือก');
	return value;
}

export function newLeavePolicyStep(): ProcessTemplateStep {
	return {
		title: '',
		kind: 'approve',
		completionPolicy: 'all',
		recipients: [],
		requiresSignature: false,
		requiresComment: false,
		requiresEvidence: false,
		requiresDelegatedAuthority: false,
		dueAfterDays: null
	};
}
export function leavePolicyDraft(kind: LeaveKind, initial?: LeavePolicyVersion) {
	const unit: LeavePolicyUnit = kind === 'exit' ? 'minute' : 'day';
	return {
		effectiveFrom: undefined as string | undefined,
		sourceReference: initial?.sourceReference ?? '',
		personnelTypes: [...(initial?.personnelTypes ?? [])] as StaffPersonnelType[],
		enabled: initial?.enabled ?? true,
		unit,
		countingMode: initial?.countingMode ?? ('calendar_days' as const),
		weekdays: [...(initial?.workingWeekdays ?? [])],
		excludedDates: [...(initial?.excludedDates ?? [])],
		quotaMonth: initial?.quotaYearStartMonth ?? 1,
		maxPerRequest: initial
			? displayedPolicyLimit(initial.maxPerRequest, initial.unit)
			: kind === 'exit'
				? '120'
				: '',
		annualLimit: displayedPolicyLimit(initial?.annualLimit, initial?.unit ?? unit),
		evidenceRequired: initial?.evidenceRequired ?? false,
		evidenceFromAmount: displayedPolicyLimit(initial?.evidenceFromAmount, initial?.unit ?? unit),
		minimumNoticeDays: initial?.minimumNoticeDays == null ? '' : String(initial.minimumNoticeDays),
		steps: initial?.steps.map((step) => ({
			...step,
			recipients: step.recipients?.map((recipient) => ({ ...recipient }))
		})) ?? [newLeavePolicyStep()]
	};
}
