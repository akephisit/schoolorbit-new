import type {
	TDocumentDefinitions,
	CustomTableLayout,
	TableCell,
	Content
} from 'pdfmake/interfaces';
import type { TimetableEntry } from '$lib/api/timetable';
import { getSchoolSettings } from '$lib/api/school';

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
// padding: ลดจาก pdfmake default (4pt) เหลือ 2pt → cells ชิดขอบมากขึ้น
const tableLayout: CustomTableLayout = {
	hLineWidth: (i, node) => (i === 0 || i === node.table.body.length ? 1 : 1),
	vLineWidth: (i, node) => (i === 0 || i === (node.table.widths?.length ?? 0) ? 1 : 1),
	hLineColor: () => '#9ca3af',
	vLineColor: () => '#9ca3af',
	paddingLeft: () => 2,
	paddingRight: () => 2,
	paddingTop: () => 2,
	paddingBottom: () => 2,
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
	/** room_id → name_th — ใช้แสดงชื่อห้องเต็ม (เช่น "ห้องคณิตศาสตร์ 1")
	 *  ถ้าไม่ส่งมาจะ fallback ไปใช้ entry.room_code (รหัสสั้น) */
	roomNames?: Record<string, string>;
}

/** Fetch image และแปลงเป็น base64 data URL — pdfmake รับเฉพาะ data URL
 *  cache: 'no-store' — กัน browser cache เก่าที่ไม่มี CORS header (หลังตั้ง CORS ตอนหลัง) */
async function fetchImageDataUrl(url: string): Promise<string | null> {
	try {
		const res = await fetch(url, { cache: 'no-store' });
		if (!res.ok) return null;
		const blob = await res.blob();
		return await new Promise<string>((resolve, reject) => {
			const reader = new FileReader();
			reader.onloadend = () => resolve(reader.result as string);
			reader.onerror = reject;
			reader.readAsDataURL(blob);
		});
	} catch {
		return null;
	}
}

function buildPageContent(
	page: TimetablePage,
	isFirst: boolean,
	logoDataUrl: string | null
): Content[] {
	const { title, subTitle, periods, timetableEntries, viewMode = 'CLASSROOM', roomNames } = page;
	const tableBody: TableCell[][] = [];

	// Header Row
	const headerRow: TableCell[] = [
		{ text: 'วัน / เวลา', bold: true, alignment: 'center', fillColor: '#f3f4f6', margin: [0, 1] }
	];
	// ใส่ \n เพื่อรักษา line height ให้คอลัมน์ที่ไม่มีชื่อสูงเท่ากับคอลัมน์ที่มีชื่อ
	// (ตรงกับ behavior ของหน้าจัดตารางที่ใช้ nbsp placeholder)
	periods.forEach((p) => {
		const labelText = p.name && p.name.trim() ? p.name : ' ';
		headerRow.push({
			text: [
				{ text: `${labelText}\n`, bold: true, fontSize: 8 },
				{
					text: `${formatTime(p.start_time)} - ${formatTime(p.end_time)}`,
					fontSize: 7,
					color: '#4b5563'
				}
			],
			alignment: 'center',
			fillColor: '#f3f4f6',
			margin: [0, 1]
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
			fontSize: 10,
			margin: [0, 2]
		});

		periods.forEach((p) => {
			const entry = getEntry(timetableEntries, day.value, p.id);
			if (entry) {
				const stack: Content[] = [];

				if (entry.entry_type === 'COURSE') {
					stack.push(
						{ text: entry.subject_code || '', bold: true, fontSize: 8, color: '#1e3a8a' },
						{
							text: entry.subject_name_th || entry.subject_name_en || 'วิชา',
							fontSize: 7,
							margin: [0, 0]
						}
					);
				} else {
					stack.push({
						text: entry.title || 'กิจกรรม',
						bold: true,
						fontSize: 8,
						color: '#047857',
						margin: [0, 2]
					});
				}

				// SLOT-sync activity: ห้อง+ครูไม่ได้ผูกกัน 1-to-1 (sync = ทุกห้องร่วมกัน,
				// ครูหลายคน) — ซ่อน meta side ที่ไม่ตรงกับ view เพื่อกัน confusion
				const isSlotSync =
					entry.entry_type === 'ACTIVITY' &&
					entry.activity_scheduling_mode === 'synchronized';

				if (viewMode === 'CLASSROOM') {
					// Student PDF — แสดงชื่อครู (ยกเว้น sync activity เพราะมีหลายครู)
					if (
						!isSlotSync &&
						entry.instructor_name &&
						entry.instructor_name.trim() &&
						entry.instructor_name !== '-'
					) {
						const rawName = entry.instructor_name.trim();
						const teacherName = rawName.startsWith('ครู') ? rawName : `ครู${rawName}`;
						stack.push({ text: teacherName, fontSize: 7, color: '#4b5563', margin: [0, 1] });
					}
				} else {
					// Teacher PDF — แสดงห้อง (ยกเว้น sync activity เพราะเป็นกิจกรรมรวมทุกห้อง)
					if (!isSlotSync && entry.classroom_name) {
						stack.push({
							text: entry.classroom_name,
							fontSize: 7,
							color: '#d97706',
							bold: true,
							margin: [0, 1]
						});
					}
				}

				// ชื่อห้องเต็ม (name_th) ถ้ามี — ไม่งั้น fallback เป็นรหัสสั้น (code)
				// name_th มักมีคำว่า "ห้อง" อยู่ในชื่ออยู่แล้ว → ไม่ใส่ prefix
				const roomFullName = entry.room_id ? roomNames?.[entry.room_id] : undefined;
				const roomDisplay = roomFullName
					? roomFullName
					: entry.room_code
						? `ห้อง ${entry.room_code}`
						: null;
				if (roomDisplay) {
					stack.push({
						text: roomDisplay,
						fontSize: 7,
						background: '#f3f4f6',
						color: '#1f2937',
						margin: [0, 2]
					});
				}

				row.push({ stack, alignment: 'center', margin: [0, 0] });
			} else {
				row.push({ text: '' });
			}
		});

		tableBody.push(row);
	});

	const titleBlock: Content = logoDataUrl
		? {
				columns: [
					// fit แทน width+height → คงสัดส่วน logo (ไม่บิดเบี้ยว)
					// portrait → ~33×50, landscape → 50×33, square → 50×50
					// wrap ใน stack เพื่อให้ width:50 ตีความเป็น column width (ไม่ใช่ image width)
					{
						width: 50,
						stack: [{ image: logoDataUrl, fit: [50, 50], alignment: 'center' }]
					},
					{
						stack: [
							{ text: title, style: 'header', alignment: 'center' },
							{ text: subTitle, style: 'subheader', alignment: 'center', margin: [0, 2, 0, 0] }
						],
						width: '*'
					},
					{ text: '', width: 50 } // balance
				],
				columnGap: 10,
				margin: [0, 0, 0, 15],
				...(isFirst ? {} : { pageBreak: 'before' })
			}
		: {
				stack: [
					{ text: title, style: 'header', alignment: 'center', margin: [0, 0, 0, 5] },
					{ text: subTitle, style: 'subheader', alignment: 'center', margin: [0, 0, 0, 15] }
				],
				...(isFirst ? {} : { pageBreak: 'before' })
			};

	return [
		titleBlock,
		{
			table: {
				headerRows: 1,
				// Fixed widths — `widths` ของ pdfmake = CONTENT width per cell
				// tableWidth จริง = sum(widths) + offsetsTotal
				// offsetsTotal = (paddingL + paddingR) × N + vLineWidth × (N+1)
				// ดู: pdfmake/src/TableProcessor.js:100 + DocMeasure.js:596-614
				widths: (() => {
					const N = periods.length + 1; // จำนวนคอลัมน์
					const padLR = 2 + 2; // จาก tableLayout paddingLeft + paddingRight
					const border = 1; // จาก tableLayout vLineWidth
					const offsetsTotal = padLR * N + border * (N + 1);
					const pageContent = 841.89 - 10 - 10; // A4 landscape - pageMargins L/R
					const safety = 2; // กันเศษ rounding
					const maxSumWidths = pageContent - offsetsTotal - safety;
					const dayCol = 40;
					const periodWidth = (maxSumWidths - dayCol) / Math.max(1, periods.length);
					return [dayCol, ...periods.map(() => periodWidth)];
				})(),
				heights: ['auto', 50, 50, 50, 50, 50],
				body: tableBody,
				dontBreakRows: true
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

	// Fetch โลโก้โรงเรียน (ถ้ามี) เป็น base64 data URL
	let logoDataUrl: string | null = null;
	try {
		const settings = await getSchoolSettings();
		if (settings.logoUrl) {
			logoDataUrl = await fetchImageDataUrl(settings.logoUrl);
		}
	} catch {
		/* ไม่มี logo ก็ไม่เป็นไร */
	}

	const content: Content[] = pages.flatMap((page, i) => buildPageContent(page, i === 0, logoDataUrl));

	const docDefinition: TDocumentDefinitions = {
		pageSize: 'A4',
		pageOrientation: 'landscape',
		// ลด margin ซ้าย-ขวาเหลือน้อยสุด → table มีที่ให้กว้างที่สุด
		pageMargins: [10, 30, 10, 20],
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
