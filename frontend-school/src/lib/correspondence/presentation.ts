import type {
	CorrespondenceSummary,
	ProcessAction,
	ProcessTemplateStep
} from '#lib/api/correspondence.js';

export const registerLabels: Record<CorrespondenceSummary['register'], string> = {
	incoming: 'หนังสือรับ',
	outgoing: 'หนังสือส่ง',
	internal: 'หนังสือภายใน'
};
export const statusLabels: Record<CorrespondenceSummary['status'], string> = {
	draft: 'ฉบับร่าง',
	in_progress: 'กำลังดำเนินการ',
	revision_requested: 'รอแก้ไข',
	completed: 'ดำเนินการแล้ว',
	rejected: 'ไม่อนุมัติ',
	closed: 'ปิดเรื่อง'
};
export const actionLabels: Record<ProcessAction, string> = {
	read: 'เปิดอ่าน',
	route: 'พิจารณาสั่งการ',
	acknowledge: 'รับทราบ',
	review: 'เสนอความเห็น',
	approve: 'อนุมัติ',
	execute: 'ดำเนินการแล้ว',
	sign: 'ลงนาม',
	reject: 'ไม่อนุมัติ',
	return_for_revision: 'ส่งกลับแก้ไข',
	report: 'ส่งผลการดำเนินงาน',
	close: 'ปิดเรื่อง',
	reassign: 'ส่งต่อผู้รับผิดชอบ'
};
export const stepLabels: Record<ProcessTemplateStep['kind'], string> = {
	acknowledge: 'รับทราบ',
	review: 'พิจารณา / เสนอความเห็น',
	approve: 'อนุมัติ',
	execute: 'ดำเนินงาน',
	sign: 'ลงนาม'
};
export function dateLabel(value?: string | null): string {
	return value
		? new Date(value).toLocaleString('th-TH', { dateStyle: 'medium', timeStyle: 'short' })
		: '—';
}
export function saveBlob(blob: Blob, filename: string): void {
	const url = URL.createObjectURL(blob);
	const link = document.createElement('a');
	link.href = url;
	link.download = filename;
	link.click();
	setTimeout(() => URL.revokeObjectURL(url), 1000);
}
