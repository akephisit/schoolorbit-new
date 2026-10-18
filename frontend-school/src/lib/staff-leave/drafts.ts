import type {
	LeavePeriodInput,
	LeaveRequestDetail,
	LeavePolicyVersion,
	LeaveUnit,
	LeaveKind,
	ProcessTemplateStep
} from '#lib/api/staff-leave.js';
import type { StaffPersonnelType } from '#lib/forms/staff-career.js';

export interface LeaveDraft {
	typeId: string;
	startDate?: string;
	endDate?: string;
	startTime: string;
	endTime: string;
	dayPortion: LeavePeriodInput['dayPortion'];
	reason: string;
	evidenceFileIds: string[];
}
export function leaveDraft(detail?: LeaveRequestDetail): LeaveDraft {
	const source = detail?.request;
	return {
		typeId: source?.typeId ?? '',
		startDate: source?.startDate,
		endDate: source?.endDate,
		startTime: source?.startTime?.slice(0, 5) ?? '',
		endTime: source?.endTime?.slice(0, 5) ?? '',
		dayPortion: source?.dayPortion ?? 'full_day',
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
	if (
		policy?.unit === 'half_day' &&
		draft.dayPortion !== 'full_day' &&
		draft.endDate !== draft.startDate
	)
		errors.endDate = 'การลาครึ่งวันต้องอยู่ในวันเดียวกัน';
	return errors;
}
export function leavePeriodInput(draft: LeaveDraft, unit: LeaveUnit): LeavePeriodInput {
	return {
		typeId: draft.typeId,
		startDate: draft.startDate ?? '',
		endDate: draft.endDate ?? '',
		startTime: unit === 'minute' ? `${draft.startTime}:00` : null,
		endTime: unit === 'minute' ? `${draft.endTime}:00` : null,
		dayPortion: unit === 'half_day' ? draft.dayPortion : 'full_day',
		reason: draft.reason.trim(),
		evidenceFileIds: draft.evidenceFileIds
	};
}
export function displayedPolicyLimit(amount: number | null | undefined, unit: LeaveUnit): string {
	return amount == null ? '' : String(unit === 'half_day' ? amount / 2 : amount);
}
export function policyLimitInput(raw: string, unit: LeaveUnit): number | null {
	if (!raw.trim()) return null;
	const value = Number(raw) * (unit === 'half_day' ? 2 : 1);
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
	const unit = initial?.unit ?? (kind === 'exit' ? 'minute' : 'half_day');
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
			? displayedPolicyLimit(initial.maxPerRequest, unit)
			: kind === 'exit'
				? '120'
				: '',
		annualLimit: displayedPolicyLimit(initial?.annualLimit, unit),
		evidenceRequired: initial?.evidenceRequired ?? false,
		evidenceFromAmount: displayedPolicyLimit(initial?.evidenceFromAmount, unit),
		minimumNoticeDays: initial?.minimumNoticeDays == null ? '' : String(initial.minimumNoticeDays),
		steps: initial?.steps.map((step) => ({
			...step,
			recipients: step.recipients?.map((recipient) => ({ ...recipient }))
		})) ?? [newLeavePolicyStep()]
	};
}
