import type { CellValue } from 'exceljs';

export type SpreadsheetValue = string | number | boolean | null | undefined;

export interface SpreadsheetSheet {
	name: string;
	rows: SpreadsheetValue[][];
	widths?: number[];
}

export async function buildXlsxWorkbook(sheets: SpreadsheetSheet[]): Promise<Uint8Array> {
	const { default: ExcelJS } = await import('exceljs');
	const workbook = new ExcelJS.Workbook();
	for (const sheet of sheets) {
		const worksheet = workbook.addWorksheet(sheet.name);
		worksheet.addRows(sheet.rows);
		worksheet.eachRow((row) => {
			row.font = { name: 'Calibri', size: 12, family: 2, scheme: 'minor', color: { theme: 1 } };
		});
		// Preserve the previous exports' character widths: 6px digits plus 5px cell padding.
		if (sheet.widths) {
			worksheet.columns = sheet.widths.map((width) => ({
				width: Math.floor((width + 5 / 6) * 256) / 256
			}));
		}
	}
	return new Uint8Array(await workbook.xlsx.writeBuffer());
}

export async function downloadXlsxWorkbook(
	sheets: SpreadsheetSheet[],
	filename: string
): Promise<void> {
	const bytes = await buildXlsxWorkbook(sheets);
	const url = URL.createObjectURL(
		new Blob([Uint8Array.from(bytes).buffer], {
			type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet'
		})
	);
	try {
		const anchor = document.createElement('a');
		anchor.href = url;
		anchor.download = filename;
		anchor.click();
	} finally {
		URL.revokeObjectURL(url);
	}
}

function scalarValue(value: CellValue): SpreadsheetValue | Date {
	if (value === null || value === undefined || typeof value !== 'object' || value instanceof Date) {
		return value;
	}
	if ('richText' in value) return value.richText.map((part) => part.text).join('');
	if ('text' in value) return value.text;
	if ('formula' in value || 'sharedFormula' in value) return scalarValue(value.result);
	return '';
}

export async function readXlsxSheets(
	bytes: ArrayBuffer | Uint8Array
): Promise<Array<{ name: string; rows: string[][] }>> {
	const [{ default: ExcelJS }, { default: SSF }] = await Promise.all([
		import('exceljs'),
		import('ssf')
	]);
	const workbook = new ExcelJS.Workbook();
	await workbook.xlsx.load(Uint8Array.from(new Uint8Array(bytes)).buffer);
	const date1904 = workbook.properties.date1904 ?? false;
	return workbook.worksheets.map((worksheet) => {
		const lastColumn = worksheet.columnCount;
		let firstColumn = lastColumn;
		worksheet.eachRow((row) => {
			row.eachCell((_, column) => {
				firstColumn = Math.min(firstColumn, column);
			});
		});
		const rows: string[][] = [];
		worksheet.eachRow((row) => {
			const values: string[] = [];
			for (let column = firstColumn; column <= lastColumn; column += 1) {
				const cell = row.getCell(column);
				let value = scalarValue(cell.value);
				if (value instanceof Date) {
					const epoch = date1904 ? Date.UTC(1904, 0, 1) : Date.UTC(1899, 11, 30);
					value = (value.getTime() - epoch) / 86_400_000;
					if (!date1904 && value < 61) value -= 1;
				}
				values.push(
					value === null || value === undefined
						? ''
						: SSF.format(cell.numFmt || 'General', value, { date1904 })
				);
			}
			if (values.some((value) => value.trim())) rows.push(values);
		});
		return { name: worksheet.name, rows };
	});
}

export function parseUtf8Csv(bytes: Uint8Array): string[][] {
	let content: string;
	try {
		content = new TextDecoder('utf-8', { fatal: true }).decode(bytes).replace(/^\uFEFF/u, '');
	} catch {
		throw new Error('ไฟล์ CSV ต้องเข้ารหัสเป็น UTF-8');
	}
	const rows: string[][] = [];
	let row: string[] = [];
	let field = '';
	let inQuotes = false;
	let closedQuote = false;
	const pushField = () => {
		row.push(field);
		field = '';
		closedQuote = false;
	};
	const pushRow = () => {
		pushField();
		rows.push(row);
		row = [];
	};
	for (let index = 0; index < content.length; index += 1) {
		const character = content[index];
		if (inQuotes) {
			if (character === '"') {
				if (content[index + 1] === '"') {
					field += '"';
					index += 1;
				} else {
					inQuotes = false;
					closedQuote = true;
				}
			} else field += character;
			continue;
		}
		if (closedQuote && ![',', '\r', '\n'].includes(character)) {
			throw new Error('มีอักขระหลังเครื่องหมายคำพูดปิดในไฟล์ CSV');
		}
		if (character === '"') {
			if (field) throw new Error('ใช้เครื่องหมายคำพูดในไฟล์ CSV ไม่ถูกต้อง');
			inQuotes = true;
		} else if (character === ',') pushField();
		else if (character === '\r' || character === '\n') {
			if (character === '\r' && content[index + 1] === '\n') index += 1;
			pushRow();
		} else field += character;
	}
	if (inQuotes) throw new Error('ปิดเครื่องหมายคำพูดในไฟล์ CSV ไม่ครบ');
	if (field || row.length > 0 || closedQuote) pushRow();
	return rows;
}

export async function readSpreadsheetFile(file: File): Promise<string[][]> {
	const extension = file.name.split('.').pop()?.toLowerCase();
	if (extension !== 'xlsx' && extension !== 'csv') {
		throw new Error('รองรับเฉพาะไฟล์ .xlsx และ .csv แบบ UTF-8 กรุณาแปลงไฟล์ .xls ก่อนนำเข้า');
	}
	const bytes = new Uint8Array(await file.arrayBuffer());
	if (extension === 'csv') return parseUtf8Csv(bytes);
	return (await readXlsxSheets(bytes))[0]?.rows ?? [];
}
