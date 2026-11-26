import type {
	LeavePeriodInput,
	LeavePolicyPreset,
	LeaveRequestDetail,
	LeaveRoutingUser,
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
	destination: string;
	companions: LeaveRoutingUser[];
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
		destination: source?.destination ?? '',
		companions: (source?.companions ?? []).map((user) => ({ ...user })),
		reason: source?.reason ?? '',
		evidenceFileIds: [...(detail?.evidenceFileIds ?? [])]
	};
}
export function leaveDraftErrors(
	draft: LeaveDraft,
	kind: LeaveKind
): Partial<Record<keyof LeaveDraft, string>> {
	const errors: Partial<Record<keyof LeaveDraft, string>> = {};
	if (!draft.typeId) errors.typeId = 'เลือกประเภทคำขอ';
	if (!draft.startDate) errors.startDate = 'เลือกวันที่เริ่ม';
	if (!draft.endDate) errors.endDate = 'เลือกวันที่สิ้นสุด';
	if (draft.startDate && draft.endDate && draft.endDate < draft.startDate)
		errors.endDate = 'วันที่สิ้นสุดต้องไม่น้อยกว่าวันที่เริ่ม';
	if (!draft.reason.trim()) errors.reason = 'ระบุเหตุผล';
	if (kind === 'exit') {
		if (!draft.startTime) errors.startTime = 'ระบุเวลาเริ่ม';
		if (!draft.endTime || draft.endTime <= draft.startTime)
			errors.endTime = 'เวลาสิ้นสุดต้องอยู่หลังเวลาเริ่ม';
		if (draft.endDate !== draft.startDate)
			errors.endDate = 'เวลาออกและเวลากลับต้องอยู่ในวันเดียวกัน';
	}
	if (kind === 'official_travel') {
		const destination = draft.destination.trim();
		if (!destination) errors.destination = 'ระบุสถานที่ไปราชการ';
		else if ([...destination].length > 500) errors.destination = 'ระบุสถานที่ไม่เกิน 500 ตัวอักษร';
		if (draft.endTime && !draft.startTime) errors.startTime = 'ระบุเวลาออกเดินทางด้วย';
		if (draft.startTime && !draft.endTime) errors.endTime = 'ระบุเวลาสิ้นสุดการเดินทางด้วย';
		if (
			draft.startDate === draft.endDate &&
			draft.startTime &&
			draft.endTime &&
			draft.endTime <= draft.startTime
		)
			errors.endTime = 'เวลาสิ้นสุดต้องอยู่หลังเวลาเริ่ม';
		if (
			draft.startDate &&
			draft.endDate &&
			Date.parse(draft.endDate) - Date.parse(draft.startDate) > 366 * 86_400_000
		)
			errors.endDate = 'ช่วงเดินทางต้องไม่เกิน 367 วัน';
	}

	return errors;
}
export function leavePeriodInput(draft: LeaveDraft, kind: LeaveKind): LeavePeriodInput {
	return {
		typeId: draft.typeId,
		academicYearId: draft.academicYearId || null,
		academicTermId: draft.academicTermId || null,
		startDate: draft.startDate ?? '',
		endDate: draft.endDate ?? '',
		startTime: kind !== 'leave' && draft.startTime ? `${draft.startTime}:00` : null,
		endTime: kind !== 'leave' && draft.endTime ? `${draft.endTime}:00` : null,
		reason: draft.reason.trim(),
		evidenceFileIds: draft.evidenceFileIds,
		...(kind === 'official_travel' ? { destination: draft.destination.trim() } : {}),
		...(kind !== 'leave' && draft.companions.length
			? { companionUserIds: draft.companions.map((user) => user.id) }
			: {})
	};
}
/** Unsafe wire numbers have already lost their original integer; never divide or copy them. */
export function displayedPolicyLimit(
	amount: number | null | undefined,
	unit: LeaveUnit
): string | undefined {
	if (amount == null) return '';
	if (!Number.isSafeInteger(amount)) return undefined;
	return String(unit === 'half_day' ? amount / 2 : amount);
}
export function policyLimitInput(raw: string): number | null {
	if (!raw.trim()) return null;
	const value = Number(raw);
	if (!/^\d+$/.test(raw.trim()) || !Number.isSafeInteger(value) || value < 1)
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
export function leavePolicyDraft(
	kind: LeaveKind,
	initial?: LeavePolicyVersion,
	preset?: LeavePolicyPreset
) {
	const unit: LeavePolicyUnit = preset?.unit ?? (kind === 'exit' ? 'minute' : 'day');
	const maxPerRequest = initial
		? displayedPolicyLimit(initial.maxPerRequest, initial.unit)
		: kind === 'exit' && !preset
			? '120'
			: '';
	const annualLimit = displayedPolicyLimit(initial?.annualLimit, initial?.unit ?? unit);
	const evidenceFromAmount = displayedPolicyLimit(
		initial?.evidenceFromAmount,
		initial?.unit ?? unit
	);
	return {
		effectiveFrom: undefined as string | undefined,
		sourceReference: initial?.sourceReference ?? preset?.sourceReference ?? '',
		personnelTypes: [
			...(initial?.personnelTypes ?? preset?.personnelTypes ?? [])
		] as StaffPersonnelType[],
		enabled: initial?.enabled ?? true,
		unit,
		countingMode: initial?.countingMode ?? preset?.countingMode ?? ('calendar_days' as const),
		weekdays: [...(initial?.workingWeekdays ?? [])],
		excludedDates: [...(initial?.excludedDates ?? [])],
		quotaMonth:
			initial?.quotaYearStartMonth ?? (preset ? (preset.quotaYearStartMonth ?? undefined) : 1),
		maxPerRequest: maxPerRequest ?? '',
		annualLimit: annualLimit ?? '',
		evidenceRequired: initial?.evidenceRequired ?? false,
		evidenceFromAmount: evidenceFromAmount ?? '',
		unresolvedLimits: {
			maxPerRequest: maxPerRequest === undefined,
			annualLimit: annualLimit === undefined,
			evidenceFromAmount: evidenceFromAmount === undefined
		},
		minimumNoticeDays: initial?.minimumNoticeDays == null ? '' : String(initial.minimumNoticeDays),
		steps: initial?.steps.map((step) => ({
			...step,
			recipients: step.recipients?.map((recipient) => ({ ...recipient }))
		})) ?? [newLeavePolicyStep()]
	};
}
