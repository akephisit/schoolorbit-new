import type {
	LeaveKind,
	LeaveRequestSummary,
	LeaveStatus,
	LeaveUnit,
	LeaveUsageItem,
	ProcessAction
} from '#lib/api/staff-leave.js';
import type { BadgeVariant } from '#lib/components/ui/badge/index.js';

export const leaveKindLabels: Record<LeaveKind, string> = {
	leave: 'การลา',
	exit: 'ออกนอกโรงเรียน',
	official_travel: 'เดินทางไปราชการ'
};
export const leavePageTitles: Record<LeaveKind, string> = {
	...leaveKindLabels,
	exit: 'ขออนุญาตออกนอกโรงเรียน'
};
export const leaveUnitLabels: Record<LeaveUnit, string> = {
	day: 'วัน',
	half_day: 'วัน (ข้อมูลเดิมหน่วยครึ่งวัน)',
	minute: 'นาที'
};
export function leaveAmount(amount: number, unit: LeaveUnit): string {
	return `${unit === 'half_day' ? amount / 2 : amount} ${unit === 'minute' ? 'นาที' : 'วัน'}`;
}
/** Render the server-scaled report quantity without converting its integer string to Number. */
export function leaveUsageAmount(
	amount: string,
	item: Pick<LeaveUsageItem, 'unit' | 'amountScale'>
): string {
	if (item.amountScale !== 1 && item.amountScale !== 2)
		throw new Error('หน่วยย่อยของรายงานไม่ถูกต้อง');
	const value = BigInt(amount),
		scale = BigInt(item.amountScale);
	const absolute = value < 0n ? -value : value;
	const fraction = absolute % scale === 0n ? '' : '.5';
	return `${value < 0n ? '-' : ''}${absolute / scale}${fraction} ${item.unit === 'minute' ? 'นาที' : 'วัน'}`;
}
export const leaveStatusLabels: Record<LeaveStatus, string> = {
	draft: 'ฉบับร่าง',
	submitted: 'รอพิจารณา',
	returned: 'ส่งกลับแก้ไข',
	approved: 'อนุมัติแล้ว',
	rejected: 'ไม่อนุมัติ',
	cancelled: 'ยกเลิกแล้ว',
	cancellation_pending: 'รออนุมัติยกเลิก'
};
export const leaveActionLabels: Record<ProcessAction, string> = {
	route: 'ส่งต่อ',
	read: 'รับทราบ',
	acknowledge: 'รับทราบ',
	review: 'ตรวจสอบผ่าน',
	approve: 'อนุมัติ',
	execute: 'ดำเนินการแล้ว',
	sign: 'ลงนาม',
	reject: 'ไม่อนุมัติ',
	return_for_revision: 'ส่งกลับแก้ไข',
	report: 'รายงานผล',
	close: 'ปิดรายการ',
	reassign: 'เปลี่ยนผู้ดำเนินการ'
};
export function leaveHistoryLabel(action: string): string {
	const labels: Record<string, string> = {
		created: 'สร้างฉบับร่าง',
		updated: 'แก้ไขฉบับร่าง',
		submitted: 'ส่งพิจารณา',
		withdrawn: 'ถอนคำขอ',
		cancellation_requested: 'ขอยกเลิกคำขอ'
	};
	if (labels[action]) return labels[action];
	if (action.startsWith('cancellation_'))
		return `${leaveHistoryLabel(action.slice(13))}การยกเลิกคำขอ`;
	return (
		Object.entries(leaveActionLabels).find(([key]) => key === action)?.[1] ?? 'บันทึกการเปลี่ยนแปลง'
	);
}
export function leaveStatusClass(status: LeaveStatus): string {
	if (status === 'approved') return 'border-success/25 bg-success/10 text-success';
	if (status === 'returned' || status === 'cancellation_pending')
		return 'border-warning/25 bg-warning/10 text-warning';
	return '';
}
export function leaveStatusVariant(status: LeaveStatus): BadgeVariant {
	if (status === 'approved') return 'default';
	if (status === 'rejected') return 'destructive';
	if (status === 'returned' || status === 'cancellation_pending') return 'outline';
	return 'secondary';
}
export function leaveHome(kind: LeaveKind): string {
	if (kind === 'exit') return '/staff/exit-permissions';
	if (kind === 'official_travel') return '/staff/official-travel';
	return '/staff/leave';
}
export function leavePeriodCaption(
	period: Pick<LeaveRequestSummary, 'startDate' | 'endDate' | 'startTime' | 'endTime'>
): string {
	const { startDate, endDate, startTime, endTime } = period;
	const start = leaveDate(startDate);
	const startClock = startTime?.slice(0, 5);
	const endClock = endTime?.slice(0, 5);
	if (startDate === endDate)
		return startClock && endClock ? `${start} · ${startClock}–${endClock}` : start;
	const end = leaveDate(endDate);
	return startClock && endClock
		? `${start} ${startClock} ถึง ${end} ${endClock}`
		: `${start} ถึง ${end}`;
}
export function leaveDate(value: string): string {
	return new Intl.DateTimeFormat('th-TH', { dateStyle: 'medium', timeZone: 'Asia/Bangkok' }).format(
		new Date(`${value}T00:00:00+07:00`)
	);
}
export function leaveDateTime(value: string): string {
	return new Intl.DateTimeFormat('th-TH', {
		dateStyle: 'medium',
		timeStyle: 'short',
		timeZone: 'Asia/Bangkok'
	}).format(new Date(value));
}
export function schoolToday(): string {
	return new Intl.DateTimeFormat('sv-SE', {
		year: 'numeric',
		month: '2-digit',
		day: '2-digit',
		timeZone: 'Asia/Bangkok'
	}).format(new Date());
}
