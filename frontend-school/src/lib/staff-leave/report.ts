import type { LeaveUsageReport } from '#lib/api/staff-leave.js';
import { leaveAmount } from './presentation.js';

/** Keep user-controlled CSV cells literal in spreadsheet applications. */
export function leaveCsvCell(value: string | number): string {
	let text = String(value);
	const leading = text.trimStart().charAt(0);
	if (
		['=', '+', '-', '@'].includes(leading) ||
		leading.charCodeAt(0) < 32 ||
		leading.charCodeAt(0) === 127 ||
		text.charCodeAt(0) < 32
	)
		text = `'${text}`;
	return `"${text.replaceAll('"', '""')}"`;
}
export function leaveUsageCsv(report: LeaveUsageReport): string {
	const rows: Array<Array<string | number>> = [
		['ช่วงรายงาน', report.from, report.to],
		['บุคลากร', 'ประเภทคำขอ', 'รอบปีเริ่ม', 'อนุมัติแล้ว', 'รอพิจารณา', 'สิทธิที่โรงเรียนกำหนด']
	];
	for (const item of report.items)
		rows.push([
			item.displayName,
			item.typeName,
			item.quotaYearStart,
			leaveAmount(item.approvedAmount, item.unit),
			leaveAmount(item.reservedAmount, item.unit),
			item.annualLimit == null ? 'ไม่กำหนด' : leaveAmount(item.annualLimit, item.unit)
		]);
	return `\uFEFF${rows.map((row) => row.map(leaveCsvCell).join(',')).join('\r\n')}`;
}
export function validReportDate(value: string | null): string | undefined {
	if (!value || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return undefined;
	const date = new Date(`${value}T00:00:00Z`);
	return Number.isFinite(date.valueOf()) && date.toISOString().slice(0, 10) === value
		? value
		: undefined;
}
