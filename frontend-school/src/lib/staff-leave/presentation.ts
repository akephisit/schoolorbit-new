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
