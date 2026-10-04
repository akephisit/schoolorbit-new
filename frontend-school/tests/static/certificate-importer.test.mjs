import assert from 'node:assert/strict';
import { File } from 'node:buffer';
import test from 'node:test';

const FIXED_HEADERS = [
	'ประเภทผู้รับ',
	'รหัสนักเรียน',
	'ชื่อผู้ใช้บุคลากร',
	'คำนำหน้า',
	'ชื่อ',
	'นามสกุล',
	'รายการกิจกรรม',
	'รางวัลหรือบทบาท',
	'แบบเกียรติบัตร'
];

test('parses quoted multiline UTF-8 CSV into typed rows without retaining the source file', async () => {
	const { CERTIFICATE_IMPORT_HEADERS, parseCertificateCsv, parseCertificateImport } =
		await import('../../src/lib/certificates/importer.ts');
	assert.deepEqual(CERTIFICATE_IMPORT_HEADERS, FIXED_HEADERS);
	const csv =
		'\uFEFFประเภทผู้รับ,รหัสนักเรียน,ชื่อผู้ใช้บุคลากร,คำนำหน้า,ชื่อ,นามสกุล,รายการกิจกรรม,รางวัลหรือบทบาท,แบบเกียรติบัตร,ครูผู้ควบคุม\r\n' +
		'บุคคลภายนอก,,,,"กมล\nชนก",ใจดี,การแข่งขันคำคม,"วิทยากร, กรรมการ",แบบวิทยากร,"ครู ก."\r\n' +
		',,,,,,,,,\r\n';
	const bytes = new TextEncoder().encode(csv);
	const parsed = parseCertificateCsv(bytes);
	assert.equal(parsed.source, 'csv');
	assert.deepEqual(parsed.headers, [...FIXED_HEADERS, 'ครูผู้ควบคุม']);
	assert.equal(parsed.rows.length, 1);
	assert.deepEqual(parsed.rows[0], {
		recipientType: 'external',
		studentId: null,
		staffUsername: null,
		title: null,
		firstName: 'กมล\nชนก',
		lastName: 'ใจดี',
		activityItem: 'การแข่งขันคำคม',
		awardOrRole: 'วิทยากร, กรรมการ',
		templateName: 'แบบวิทยากร',
		customValues: { ครูผู้ควบคุม: 'ครู ก.' }
	});

	const file = new File([bytes], 'recipients.csv', { type: 'text/csv' });
	const fromFile = await parseCertificateImport(file);
	assert.deepEqual(fromFile, parsed);
	assert.equal(JSON.stringify(fromFile).includes('recipients.csv'), false);
});

test('rejects duplicate headers, malformed quotes, and non UTF-8 CSV atomically', async () => {
	const { parseCertificateCsv } = await import('../../src/lib/certificates/importer.ts');
	assert.throws(
		() =>
			parseCertificateCsv(
				new TextEncoder().encode('ประเภทผู้รับ,ชื่อ, ชื่อ ,นามสกุล\nบุคคลภายนอก,กมล,กมล,ใจดี')
			),
		/หัวคอลัมน์ซ้ำ/
	);
	assert.throws(
		() =>
			parseCertificateCsv(
				new TextEncoder().encode('ประเภทผู้รับ,ชื่อ,นามสกุล\nบุคคลภายนอก,"กมล,ใจดี')
			),
		/เครื่องหมายคำพูด/
	);
	assert.throws(() => parseCertificateCsv(Uint8Array.from([0xc3, 0x28])), /UTF-8/);
});

test('reads displayed XLSX strings, preserves formula-looking text, and rejects a second non-empty sheet', async () => {
	const { default: ExcelJS } = await import('exceljs');
	const { parseCertificateXlsx } = await import('../../src/lib/certificates/importer.ts');
	const workbook = new ExcelJS.Workbook();
	const sheet = workbook.addWorksheet('รายชื่อผู้รับ');
	sheet.addRows([
		FIXED_HEADERS,
		['นักเรียน', '0069', '', 'เด็กหญิง', 'กมล', 'ใจดี', '=1+1', 'ชนะเลิศ', 'แบบรางวัล'],
		['', '', '', '', '', '', '', '', '']
	]);
	sheet.getCell('B2').value = 69;
	sheet.getCell('B2').numFmt = '0000';
	const bytes = await workbook.xlsx.writeBuffer();
	const parsed = await parseCertificateXlsx(bytes);
	assert.equal(parsed.source, 'xlsx');
	assert.equal(parsed.rows.length, 1);
	assert.equal(parsed.rows[0].studentId, '0069');
	assert.equal(parsed.rows[0].activityItem, '=1+1');

	workbook.addWorksheet('ชีตที่สอง').addRows([['ชื่อ'], ['ไม่ควรถูกนำเข้า']]);
	const multiple = await workbook.xlsx.writeBuffer();
	await assert.rejects(() => parseCertificateXlsx(multiple), /มากกว่าหนึ่งชีต/);
});

test('builds CSV and XLSX examples with the fixed columns and one fictional row', async () => {
	const { default: ExcelJS } = await import('exceljs');
	const { parseCertificateCsv } = await import('../../src/lib/certificates/importer.ts');
	const { buildCertificateCsvTemplate, buildCertificateXlsxTemplate } =
		await import('../../src/lib/certificates/import-template.ts');
	const csvBytes = buildCertificateCsvTemplate();
	assert.deepEqual(Array.from(csvBytes.slice(0, 3)), [0xef, 0xbb, 0xbf]);
	const csv = parseCertificateCsv(csvBytes);
	assert.deepEqual(csv.headers, FIXED_HEADERS);
	assert.equal(csv.rows.length, 1);
	assert.equal(csv.rows[0].firstName, 'กมลชนก');

	const xlsxBytes = await buildCertificateXlsxTemplate();
	const workbook = new ExcelJS.Workbook();
	await workbook.xlsx.load(xlsxBytes);
	assert.deepEqual(workbook.worksheets[0].getRow(1).values.slice(1), FIXED_HEADERS);
});
