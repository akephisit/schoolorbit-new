import assert from 'node:assert/strict';
import { File } from 'node:buffer';
import test from 'node:test';
import ExcelJS from 'exceljs';
import {
	buildXlsxWorkbook,
	parseUtf8Csv,
	readSpreadsheetFile,
	readXlsxSheets
} from '../../src/lib/utils/spreadsheet.ts';

test('exports text IDs, literal formulas, numeric ranks, sheet names and template widths', async () => {
	const bytes = await buildXlsxWorkbook([
		{
			name: 'รายชื่อ',
			rows: [
				['เลขประจำตัว', 'ชื่อ', 'ลำดับ'],
				['0069', '=1+1', 1]
			],
			widths: [14, 20, 12]
		},
		{ name: 'ห้องสอบ', rows: [['ที่นั่ง'], [2]] }
	]);
	const workbook = new ExcelJS.Workbook();
	await workbook.xlsx.load(bytes);
	assert.deepEqual(
		workbook.worksheets.map((sheet) => sheet.name),
		['รายชื่อ', 'ห้องสอบ']
	);
	const sheet = workbook.worksheets[0];
	assert.equal(sheet.getCell('A2').value, '0069');
	assert.equal(sheet.getCell('B2').value, '=1+1');
	assert.equal(sheet.getCell('C2').value, 1);
	assert.equal(sheet.getCell('A2').font.name, 'Calibri');
	assert.equal(sheet.getCell('A2').font.size, 12);
	assert.deepEqual(
		sheet.columns.map((column) => column.width),
		[14.83203125, 20.83203125, 12.83203125]
	);
});

for (const date1904 of [false, true]) {
	test(`imports displayed values, cached formulas and dates with date1904=${date1904}`, async () => {
		const workbook = new ExcelJS.Workbook();
		workbook.properties.date1904 = date1904;
		const sheet = workbook.addWorksheet('รายชื่อ');
		sheet.addRow(['รหัส', 'ข้อความ', 'วันที่', 'ร้อยละ', 'สูตร', 'ลิงก์', 'ช่องว่าง']);
		sheet.addRow([
			69,
			{ richText: [{ text: 'ภาษา' }, { text: 'ไทย' }] },
			new Date('2026-10-04T00:00:00Z'),
			0.125,
			{ formula: '1+1', result: 2 },
			{ text: 'ตัวอย่าง', hyperlink: 'https://example.com' },
			''
		]);
		sheet.getCell('A2').numFmt = '0000';
		sheet.getCell('C2').numFmt = 'yyyy-mm-dd';
		sheet.getCell('D2').numFmt = '0.0%';
		sheet.addRow([]);
		workbook.addWorksheet('ชีตว่าง');
		const sheets = await readXlsxSheets(await workbook.xlsx.writeBuffer());
		assert.deepEqual(sheets[0].rows[1], [
			'0069',
			'ภาษาไทย',
			'2026-10-04',
			'12.5%',
			'2',
			'ตัวอย่าง',
			''
		]);
		assert.equal(sheets[0].rows.length, 2);
		assert.deepEqual(sheets[1].rows, []);
	});
}

test('CSV preserves UTF-8, leading zeros, quoted commas, escaped quotes and line endings', async () => {
	const bytes = new TextEncoder().encode(
		'\uFEFFเลขประจำตัว,ชื่อ,นามสกุล\r\n0069,"ภาษา,ไทย","ทดสอบ""ชื่อ\nใหม่"\r\n'
	);
	const expected = [
		['เลขประจำตัว', 'ชื่อ', 'นามสกุล'],
		['0069', 'ภาษา,ไทย', 'ทดสอบ"ชื่อ\nใหม่']
	];
	assert.deepEqual(parseUtf8Csv(bytes), expected);
	assert.deepEqual(await readSpreadsheetFile(new File([bytes], 'names.CSV')), expected);
	assert.throws(() => parseUtf8Csv(new TextEncoder().encode('"ชื่อ"ผิด,สกุล')), /อักขระหลัง/);
	assert.throws(() => parseUtf8Csv(Uint8Array.from([0xc3, 0x28])), /UTF-8/);
});

test('file import reads the first XLSX sheet and rejects legacy XLS and unsupported files', async () => {
	const bytes = await buildXlsxWorkbook([
		{
			name: 'รายชื่อ',
			rows: [
				['รหัส', 'ชื่อ'],
				['0069', 'ตัวอย่าง']
			]
		},
		{ name: 'อื่น', rows: [['ไม่ใช่รายชื่อ']] }
	]);
	assert.deepEqual(await readSpreadsheetFile(new File([bytes], 'names.xlsx')), [
		['รหัส', 'ชื่อ'],
		['0069', 'ตัวอย่าง']
	]);
	await assert.rejects(() => readSpreadsheetFile(new File([bytes], 'names.xls')), /แปลงไฟล์ .xls/);
	await assert.rejects(() => readSpreadsheetFile(new File([bytes], 'names.txt')), /รองรับเฉพาะ/);
	await assert.rejects(() => readXlsxSheets(new TextEncoder().encode('not a workbook')));
});
