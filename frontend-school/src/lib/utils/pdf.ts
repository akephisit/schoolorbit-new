import type {
	TDocumentDefinitions,
	CustomTableLayout,
	TableCell,
	Content
} from 'pdfmake/interfaces';
import type { TimetableEntry } from '$lib/api/timetable';

interface PdfPeriod {
	id: string;
	order_index: number;
	name?: string | null;
	start_time: string;
	end_time: string;
}

// Define fonts
// Define fonts moved inside function to avoid SSR window error

// Helper: Format time
const formatTime = (timeStr: string) => {
	if (!timeStr) return '';
	return timeStr.substring(0, 5);
};

// Helper: Get entry
const getEntry = (entries: TimetableEntry[], day: string, periodId: string) => {
	return entries.find((e) => e.day_of_week === day && e.period_id === periodId && e.is_active);
};

// Helper: Define Table Layout
const tableLayout: CustomTableLayout = {
	hLineWidth: (i, node) => (i === 0 || i === node.table.body.length ? 1 : 1),
	vLineWidth: (i, node) => (i === 0 || i === (node.table.widths?.length ?? 0) ? 1 : 1),
	hLineColor: () => '#9ca3af',
	vLineColor: () => '#9ca3af',
	fillColor: () => {
		return null;
	}
};

const DAYS = [
	{ value: 'MON', label: 'จันทร์', color: '#FEF9C3' },
	{ value: 'TUE', label: 'อังคาร', color: '#FCE7F3' },
	{ value: 'WED', label: 'พุธ', color: '#DCFCE7' },
	{ value: 'THU', label: 'พฤหัสฯ', color: '#FFEDD5' },
	{ value: 'FRI', label: 'ศุกร์', color: '#DBEAFE' },
	{ value: 'SAT', label: 'เสาร์', color: '#F3F4F6' },
	{ value: 'SUN', label: 'อาทิตย์', color: '#F3F4F6' }
];

export interface TimetablePage {
	title: string;
	subTitle: string;
	periods: PdfPeriod[];
	timetableEntries: TimetableEntry[];
	viewMode?: 'CLASSROOM' | 'INSTRUCTOR';
}

function buildPageContent(page: TimetablePage, isFirst: boolean): Content[] {
	const { title, subTitle, periods, timetableEntries, viewMode = 'CLASSROOM' } = page;
	const tableBody: TableCell[][] = [];

	// Header Row
	const headerRow: TableCell[] = [
		{ text: 'วัน / เวลา', bold: true, alignment: 'center', fillColor: '#f3f4f6', margin: [0, 5] }
	];
	// ใส่ \n เพื่อรักษา line height ให้คอลัมน์ที่ไม่มีชื่อสูงเท่ากับคอลัมน์ที่มีชื่อ
	// (ตรงกับ behavior ของหน้าจัดตารางที่ใช้ nbsp placeholder)
	periods.forEach((p) => {
		const labelText = p.name && p.name.trim() ? p.name : ' ';
		headerRow.push({
			text: [
				{ text: `${labelText}\n`, bold: true, fontSize: 9 },
				{
					text: `${formatTime(p.start_time)} - ${formatTime(p.end_time)}`,
					fontSize: 8,
					color: '#4b5563'
				}
			],
			alignment: 'center',
			fillColor: '#f3f4f6',
			margin: [0, 2]
		});
	});
	tableBody.push(headerRow);

	// Data Rows (MON - FRI)
	DAYS.slice(0, 5).forEach((day) => {
		const row: TableCell[] = [];

		row.push({
			text: day.label,
			bold: true,
			alignment: 'center',
			fillColor: day.color,
			fontSize: 12,
			margin: [0, 15]
		});

		periods.forEach((p) => {
			const entry = getEntry(timetableEntries, day.value, p.id);
			if (entry) {
				const stack: Content[] = [];

				if (entry.entry_type === 'COURSE') {
					stack.push(
						{ text: entry.subject_code || '', bold: true, fontSize: 10, color: '#1e3a8a' },
						{
							text: entry.subject_name_th || entry.subject_name_en || 'วิชา',
							fontSize: 9,
							margin: [0, 0]
						}
					);
				} else {
					stack.push({
						text: entry.title || 'กิจกรรม',
						bold: true,
						fontSize: 10,
						color: '#047857',
						margin: [0, 2]
					});
				}

				if (viewMode === 'CLASSROOM') {
					if (
						entry.instructor_name &&
						entry.instructor_name.trim() &&
						entry.instructor_name !== '-'
					) {
						const rawName = entry.instructor_name.trim();
						const teacherName = rawName.startsWith('ครู') ? rawName : `ครู${rawName}`;
						stack.push({ text: teacherName, fontSize: 8, color: '#4b5563', margin: [0, 1] });
					}
				} else {
					if (entry.classroom_name) {
						stack.push({
							text: entry.classroom_name,
							fontSize: 9,
							color: '#d97706',
							bold: true,
							margin: [0, 1]
						});
					}
				}

				if (entry.room_code) {
					stack.push({
						text: `ห้อง ${entry.room_code}`,
						fontSize: 8,
						background: '#f3f4f6',
						color: '#1f2937',
						margin: [0, 2]
					});
				}

				row.push({ stack, alignment: 'center', margin: [2, 5] });
			} else {
				row.push({ text: '' });
			}
		});

		tableBody.push(row);
	});

	return [
		{
			text: title,
			style: 'header',
			alignment: 'center',
			margin: [0, 0, 0, 5],
			...(isFirst ? {} : { pageBreak: 'before' })
		},
		{ text: subTitle, style: 'subheader', alignment: 'center', margin: [0, 0, 0, 20] },
		{
			table: {
				headerRows: 1,
				// day column: fixed กระชับ; period columns: แชร์ space ที่เหลือเท่าๆ กัน
				// fix(overflow): ใช้ค่าตายตัวแทน 'auto' ป้องกันคอลัมน์วันกินพื้นที่จนตารางล้นขอบ
				widths: [50, ...periods.map(() => '*')],
				body: tableBody
			},
			layout: tableLayout
		},
		{
			columns: [
				{ text: `ข้อมูล ณ วันที่ ${new Date().toLocaleDateString('th-TH')}`, style: 'footer' },
				{ text: 'SchoolOrbit TimeTable', style: 'footer', alignment: 'right' }
			],
			margin: [0, 10, 0, 0]
		}
	];
}

export const generateTimetablePDF = async (pages: TimetablePage[], fileName?: string) => {
	if (pages.length === 0) return;

	const pdfMakeModule = await import('pdfmake/build/pdfmake');
	const pdfMake = pdfMakeModule.default;

	pdfMake.fonts = {
		Sarabun: {
			normal: window.location.origin + '/fonts/Sarabun-Regular.ttf',
			bold: window.location.origin + '/fonts/Sarabun-Bold.ttf',
			italics: window.location.origin + '/fonts/Sarabun-Regular.ttf',
			bolditalics: window.location.origin + '/fonts/Sarabun-Bold.ttf'
		}
	};

	const content: Content[] = pages.flatMap((page, i) => buildPageContent(page, i === 0));

	const docDefinition: TDocumentDefinitions = {
		pageSize: 'A4',
		pageOrientation: 'landscape',
		// ลด margin ซ้าย-ขวา เพื่อมีพื้นที่ตารางมากขึ้น (ลด overflow บนคาบเยอะๆ)
		pageMargins: [20, 40, 20, 30],
		content,
		styles: {
			header: { fontSize: 18, bold: true, color: '#1e3a8a' },
			subheader: { fontSize: 14, color: '#4b5563' },
			footer: { fontSize: 8, color: '#9ca3af' }
		},
		defaultStyle: { font: 'Sarabun' }
	};

	pdfMake.createPdf(docDefinition).download(`${fileName ?? pages[0].title}.pdf`);
};
