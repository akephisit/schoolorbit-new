import { parseUtf8Csv, readXlsxSheets } from '../utils/spreadsheet.ts';
import type { CertificateImportRequest } from '#lib/api/certificates.js';

export const CERTIFICATE_IMPORT_HEADERS = [
	'ประเภทผู้รับ',
	'รหัสนักเรียน',
	'ชื่อผู้ใช้บุคลากร',
	'คำนำหน้า',
	'ชื่อ',
	'นามสกุล',
	'รายการกิจกรรม',
	'รางวัลหรือบทบาท',
	'แบบเกียรติบัตร'
] as const;

export type ParsedCertificateImport = CertificateImportRequest;
type ImportRow = CertificateImportRequest['rows'][number];

const ZERO_WIDTH_CHARACTERS = /\u200b|\u200c|\u200d|\ufeff/gu;

function normalizeHeader(value: string): string {
	return value.normalize('NFC').replace(ZERO_WIDTH_CHARACTERS, '').trim().replace(/\s+/gu, ' ');
}

function normalizeHeaderKey(value: string): string {
	return normalizeHeader(value).toLocaleLowerCase('th');
}

function normalizeCell(value: unknown): string {
	return String(value ?? '')
		.normalize('NFC')
		.replace(ZERO_WIDTH_CHARACTERS, '')
		.trim();
}

function normalizeRecipientType(value: string): string {
	switch (value.toLocaleLowerCase('th')) {
		case 'นักเรียน':
		case 'student':
			return 'student';
		case 'บุคลากร':
		case 'staff':
			return 'staff';
		case 'บุคคลภายนอก':
		case 'external':
			return 'external';
		default:
			return value;
	}
}

function validateHeaders(values: string[]): string[] {
	if (values.length === 0) throw new Error('ไม่พบหัวคอลัมน์ในไฟล์รายชื่อ');
	const headers = values.map(normalizeHeader);
	if (headers.some((header) => !header)) throw new Error('หัวคอลัมน์ต้องไม่ว่าง');
	const seen = new Set<string>();
	for (const header of headers) {
		const key = normalizeHeaderKey(header);
		if (seen.has(key)) throw new Error(`พบหัวคอลัมน์ซ้ำ: ${header}`);
		seen.add(key);
	}
	return headers;
}

function optionalCell(value: string | undefined): string | null {
	const normalized = normalizeCell(value);
	return normalized || null;
}

function rowsToRequest(
	source: 'csv' | 'xlsx',
	grid: Array<Array<string | number | boolean | null | undefined>>
): ParsedCertificateImport {
	const nonEmptyRows = grid.map((row) => row.map(normalizeCell));
	while (nonEmptyRows.length > 0 && nonEmptyRows.at(-1)?.every((cell) => !cell)) {
		nonEmptyRows.pop();
	}
	if (nonEmptyRows.length === 0) throw new Error('ไฟล์รายชื่อไม่มีข้อมูล');
	const headers = validateHeaders(nonEmptyRows[0]);
	const headerIndexes = new Map(
		headers.map((header, index) => [normalizeHeaderKey(header), index])
	);
	const standardKeys = new Set(CERTIFICATE_IMPORT_HEADERS.map(normalizeHeaderKey));
	const valueAt = (row: string[], header: (typeof CERTIFICATE_IMPORT_HEADERS)[number]) =>
		row[headerIndexes.get(normalizeHeaderKey(header)) ?? -1] ?? '';
	const rows: ImportRow[] = nonEmptyRows
		.slice(1)
		.filter((row) => row.some(Boolean))
		.map((row) => {
			const customValues = Object.fromEntries(
				headers
					.map((header, index) => [header, row[index] ?? ''] as const)
					.filter(([header, value]) => !standardKeys.has(normalizeHeaderKey(header)) && value)
			);
			return {
				recipientType: normalizeRecipientType(valueAt(row, 'ประเภทผู้รับ')),
				studentId: optionalCell(valueAt(row, 'รหัสนักเรียน')),
				staffUsername: optionalCell(valueAt(row, 'ชื่อผู้ใช้บุคลากร')),
				title: optionalCell(valueAt(row, 'คำนำหน้า')),
				firstName: valueAt(row, 'ชื่อ'),
				lastName: valueAt(row, 'นามสกุล'),
				activityItem: optionalCell(valueAt(row, 'รายการกิจกรรม')),
				awardOrRole: optionalCell(valueAt(row, 'รางวัลหรือบทบาท')),
				templateName: optionalCell(valueAt(row, 'แบบเกียรติบัตร')),
				customValues
			};
		});
	if (rows.length === 0) throw new Error('ไฟล์รายชื่อไม่มีแถวข้อมูล');
	return { source, headers, rows };
}

export function parseCertificateCsv(bytes: Uint8Array): ParsedCertificateImport {
	return rowsToRequest('csv', parseUtf8Csv(bytes));
}

export async function parseCertificateXlsx(
	bytes: ArrayBuffer | Uint8Array
): Promise<ParsedCertificateImport> {
	const populatedSheets = (await readXlsxSheets(bytes)).filter(({ rows }) =>
		rows.some((row) => row.some((cell) => normalizeCell(cell)))
	);
	if (populatedSheets.length === 0) throw new Error('ไฟล์ Excel ไม่มีข้อมูล');
	if (populatedSheets.length > 1) {
		throw new Error('ไฟล์ Excel มีข้อมูลมากกว่าหนึ่งชีต กรุณาเหลือชีตเดียวก่อนนำเข้า');
	}
	return rowsToRequest('xlsx', populatedSheets[0].rows);
}

export async function parseCertificateImport(file: File): Promise<ParsedCertificateImport> {
	const extension = file.name.split('.').pop()?.toLocaleLowerCase('en-US');
	const bytes = new Uint8Array(await file.arrayBuffer());
	if (extension === 'csv') return parseCertificateCsv(bytes);
	if (extension === 'xlsx') return parseCertificateXlsx(bytes);
	throw new Error('รองรับเฉพาะไฟล์ .xlsx และ .csv แบบ UTF-8');
}
