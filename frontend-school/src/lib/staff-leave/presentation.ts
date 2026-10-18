import type { LeaveKind, LeaveStatus, LeaveUnit, ProcessAction } from '#lib/api/staff-leave.js';
import type { BadgeVariant } from '#lib/components/ui/badge/index.js';

export const leaveKindLabels: Record<LeaveKind, string> = {
	leave: 'การลา',
	exit: 'ออกนอกโรงเรียน'
};
export const leaveUnitLabels: Record<LeaveUnit, string> = {
	day: 'วัน',
	half_day: 'วัน (รองรับครึ่งวัน)',
	minute: 'นาที'
};
export function leaveAmount(amount: number, unit: LeaveUnit): string {
	return `${unit === 'half_day' ? amount / 2 : amount} ${unit === 'minute' ? 'นาที' : 'วัน'}`;
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
export function leaveStatusVariant(status: LeaveStatus): BadgeVariant {
	if (status === 'approved') return 'default';
	if (status === 'rejected') return 'destructive';
	if (status === 'returned' || status === 'cancellation_pending') return 'outline';
	return 'secondary';
}
export function leaveHome(kind: LeaveKind): string {
	return kind === 'exit' ? '/staff/exit-permissions' : '/staff/leave';
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
