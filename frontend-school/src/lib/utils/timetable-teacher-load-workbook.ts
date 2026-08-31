import type { Cell, Row, Workbook, Worksheet } from 'exceljs';

import type { TimetableEntry } from '$lib/api/timetable';
import {
	buildTeacherLoadExportRows,
	calculateTeacherLoadColumnWidths,
	TEACHER_LOAD_DETAIL_COLUMN_WIDTH_OPTIONS,
	TEACHER_LOAD_SUMMARY_COLUMN_WIDTH_OPTIONS,
	type TeacherLoadExportRows
} from '$lib/utils/timetable-teacher-load-export';

const teacherLoadFontName = 'TH Sarabun New';

function styleCell(cell: Cell, emphasized = false): void {
	cell.font = { name: teacherLoadFontName, size: 16, bold: emphasized };
	cell.alignment = { vertical: 'middle', wrapText: true };
	cell.border = {
		top: { style: 'thin', color: { argb: 'FFE2E8F0' } },
		left: { style: 'thin', color: { argb: 'FFE2E8F0' } },
		bottom: { style: 'thin', color: { argb: 'FFE2E8F0' } },
		right: { style: 'thin', color: { argb: 'FFE2E8F0' } }
	};
}

function styleRow(row: Row, kind: 'header' | 'group' | 'detail'): void {
	row.height = kind === 'header' ? 28 : 24;
	row.eachCell({ includeEmpty: true }, (cell) => {
		styleCell(cell, kind !== 'detail');
		if (kind !== 'detail') {
			cell.fill = {
				type: 'pattern',
				pattern: 'solid',
				fgColor: { argb: kind === 'header' ? 'FFE2E8F0' : 'FFF1F5F9' }
			};
		}
	});
}

function appendSheet(
	workbook: Workbook,
	name: string,
	rows: Array<Array<string | number>>,
	widthOptions:
		| typeof TEACHER_LOAD_SUMMARY_COLUMN_WIDTH_OPTIONS
		| typeof TEACHER_LOAD_DETAIL_COLUMN_WIDTH_OPTIONS
): Worksheet {
	const worksheet = workbook.addWorksheet(name);
	worksheet.columns = calculateTeacherLoadColumnWidths(rows, widthOptions).map((width) => ({
		width
	}));
	worksheet.properties.defaultRowHeight = 24;

	for (const [index, values] of rows.entries()) {
		const row = worksheet.addRow(values);
		const firstCell = String(values[0] ?? '');
		styleRow(row, index === 0 ? 'header' : firstCell.startsWith('กลุ่มสาระ:') ? 'group' : 'detail');
	}

	worksheet.views = [{ state: 'frozen', ySplit: 1 }];
	worksheet.pageSetup = {
		orientation: 'landscape',
		fitToPage: true,
		fitToWidth: 1,
		fitToHeight: 0
	};
	return worksheet;
}

function appendSheets(workbook: Workbook, rows: TeacherLoadExportRows): void {
	appendSheet(
		workbook,
		'สรุปต่อครู',
		rows.summarySheetRows,
		TEACHER_LOAD_SUMMARY_COLUMN_WIDTH_OPTIONS
	);
	appendSheet(
		workbook,
		'รายละเอียด',
		rows.detailSheetRows,
		TEACHER_LOAD_DETAIL_COLUMN_WIDTH_OPTIONS
	);
}

function safeFileName(value: string): string {
	return (
		value
			.replace(/[\\/:*?"<>|]/g, '-')
			.replace(/\s+/g, ' ')
			.trim() || 'สรุปคาบสอนครู'
	);
}

function saveBuffer(buffer: ArrayBuffer, fileName: string): void {
	const blob = new Blob([buffer], {
		type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet'
	});
	const url = URL.createObjectURL(blob);
	const link = document.createElement('a');
	link.href = url;
	link.download = fileName;
	document.body.appendChild(link);
	link.click();
	link.remove();
	URL.revokeObjectURL(url);
}

export async function downloadTeacherLoadWorkbook(
	entries: TimetableEntry[],
	fileLabel: string
): Promise<number> {
	const rows = buildTeacherLoadExportRows(entries);
	if (rows.summaryRows.length === 0) return 0;

	const ExcelJSModule = await import('exceljs');
	const ExcelJS = ExcelJSModule.default;
	const workbook = new ExcelJS.Workbook();
	workbook.creator = 'SchoolOrbit';
	workbook.created = new Date();
	workbook.modified = new Date();
	appendSheets(workbook, rows);
	const buffer = await workbook.xlsx.writeBuffer();
	saveBuffer(buffer, `${safeFileName(fileLabel)}.xlsx`);
	return rows.summaryRows.length;
}
