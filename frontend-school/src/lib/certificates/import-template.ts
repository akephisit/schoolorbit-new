import { buildXlsxWorkbook } from '../utils/spreadsheet.ts';
import { CERTIFICATE_IMPORT_HEADERS } from './importer.ts';

const FICTIONAL_ROW = [
	'บุคคลภายนอก',
	'',
	'',
	'คุณ',
	'กมลชนก',
	'ใจดี',
	'กิจกรรมวันภาษาไทย',
	'วิทยากร',
	'แบบวิทยากร'
];

function csvCell(value: string): string {
	return /[",\r\n]/u.test(value) ? `"${value.replaceAll('"', '""')}"` : value;
}

export function buildCertificateCsvTemplate(): Uint8Array {
	const content = [CERTIFICATE_IMPORT_HEADERS, FICTIONAL_ROW]
		.map((row) => row.map(csvCell).join(','))
		.join('\r\n');
	return new TextEncoder().encode(`\uFEFF${content}\r\n`);
}

export async function buildCertificateXlsxTemplate(): Promise<Uint8Array> {
	return buildXlsxWorkbook([
		{
			name: 'รายชื่อผู้รับ',
			rows: [Array.from(CERTIFICATE_IMPORT_HEADERS), FICTIONAL_ROW],
			widths: CERTIFICATE_IMPORT_HEADERS.map((header) =>
				Math.max(16, Array.from(header).length + 4)
			)
		}
	]);
}

function downloadBytes(bytes: Uint8Array, filename: string, type: string): void {
	const payload = Uint8Array.from(bytes).buffer;
	const url = URL.createObjectURL(new Blob([payload], { type }));
	const anchor = document.createElement('a');
	anchor.href = url;
	anchor.download = filename;
	anchor.click();
	URL.revokeObjectURL(url);
}

export function downloadCertificateCsvTemplate(): void {
	downloadBytes(
		buildCertificateCsvTemplate(),
		'ตัวอย่างรายชื่อเกียรติบัตร.csv',
		'text/csv;charset=utf-8'
	);
}

export async function downloadCertificateXlsxTemplate(): Promise<void> {
	downloadBytes(
		await buildCertificateXlsxTemplate(),
		'ตัวอย่างรายชื่อเกียรติบัตร.xlsx',
		'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet'
	);
}
