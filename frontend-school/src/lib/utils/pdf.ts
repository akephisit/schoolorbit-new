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

/** เลือกรูปแบบเวลาที่พอดีกับ cellContentWidth (single line ก่อน, fallback 2 บรรทัด)
 *  ลำดับ:
 *  1. "08:40-09:30 น." @ 7pt  (มี "น.")
 *  2. "08:40-09:30" @ 7pt     (ตัด "น.")
 *  3. "08:40-09:30" @ 6pt     (ลด font)
 *  4. "08:40-09:30" @ 5pt     (ลด font อีก)
 *  5. "08:40\n09:30" @ 7pt    (fallback 2 บรรทัด) */
function fitTimeRange(
	start: string,
	end: string,
	maxWidthPt: number
): { text: string; fontSize: number } {
	const s = formatTime(start);
	const e = formatTime(end);

	const variants: { text: string; fontSize: number }[] = [
		{ text: `${s}-${e} น.`, fontSize: 7 },
		{ text: `${s}-${e}`, fontSize: 7 },
		{ text: `${s}-${e}`, fontSize: 6 },
		{ text: `${s}-${e}`, fontSize: 5 }
	];

	for (const v of variants) {
		if (measureTextWidthPt(v.text, v.fontSize) <= maxWidthPt) {
			return v;
		}
	}
	return { text: `${s}\n${e}`, fontSize: 7 };
}

// Helper: Get entry
const getEntry = (entries: TimetableEntry[], day: string, periodId: string) => {
	return entries.find((e) => e.day_of_week === day && e.period_id === periodId && e.is_active);
};

// Helper: Strip "ตารางเรียน ชั้น" / "ตารางสอน " prefix → ใช้เป็น mini-title ใน grid mode
const stripTitlePrefix = (title: string): string => {
	return title.replace(/^ตารางเรียน ชั้น/, '').replace(/^ตารางสอน /, '');
};

type SegmenterCtor = new (
	locale: string,
	opts: { granularity: 'grapheme' | 'word' | 'sentence' }
) => { segment(text: string): Iterable<{ segment: string }> };

/** Canvas-based text measurement — ใช้ Sarabun font (โหลดผ่าน FontFace API)
 *  เพื่อวัด width จริงที่ pdfmake จะ render → แม่นกว่า approximation
 *  fallback ไป approxTextWidthPt ถ้า font ยังโหลดไม่เสร็จหรือไม่มี DOM */
let measureCanvas: HTMLCanvasElement | null = null;
let measureCtx: CanvasRenderingContext2D | null = null;
let sarabunMeasureReady = false;

async function ensureSarabunMeasurement(): Promise<void> {
	if (sarabunMeasureReady || typeof document === 'undefined') return;
	try {
		const url = window.location.origin + '/fonts/Sarabun-Regular.ttf';
		const font = new FontFace('SarabunMeasure', `url(${url}) format('truetype')`);
		await font.load();
		document.fonts.add(font);
		sarabunMeasureReady = true;
	} catch {
		// ignore — จะใช้ approx แทน
	}
}

const PT_PER_PX = 72 / 96; // 1pt = 1.333px (CSS standard)

function measureTextWidthPt(text: string, fontSizePt: number): number {
	if (!text) return 0;
	if (!sarabunMeasureReady || typeof document === 'undefined') {
		return approxTextWidthPt(text, fontSizePt);
	}
	if (!measureCtx) {
		measureCanvas = document.createElement('canvas');
		measureCtx = measureCanvas.getContext('2d');
		if (!measureCtx) return approxTextWidthPt(text, fontSizePt);
	}
	const fontSizePx = fontSizePt / PT_PER_PX;
	measureCtx.font = `${fontSizePx}px SarabunMeasure, sans-serif`;
	const widthPx = measureCtx.measureText(text).width;
	return widthPx * PT_PER_PX;
}

/** Thai syllable split — ตัดก่อน leading vowel (ใไเแโ) → cleaner sub-breaks
 *  เช่น "ไฮโดรโปนิกส์" → ["ไฮ", "โดร", "โปนิกส์"] (3 syllables)
 *  ดีกว่า grapheme split (ตัดทุกตัว) สำหรับคำที่ ICU dict ไม่รู้จัก */
function syllableSplit(text: string): string[] {
	const LEADING_VOWELS = new Set(['ใ', 'ไ', 'เ', 'แ', 'โ']);
	const result: string[] = [];
	let current = '';
	for (const ch of text) {
		if (LEADING_VOWELS.has(ch) && current.length > 0) {
			result.push(current);
			current = ch;
		} else {
			current += ch;
		}
	}
	if (current) result.push(current);
	return result;
}

/** ประมาณ width (pt) ของ text ใน Sarabun font — fallback เมื่อ canvas measurement ไม่พร้อม
 *  ใช้ grapheme-cluster counting (Intl.Segmenter) เพื่อ handle Thai composed chars
 *  ค่าประมาณ: Thai grapheme ~0.55em, Latin ~0.5em, digit ~0.55em, space ~0.25em
 *  + safety factor 1.1 */
function approxTextWidthPt(text: string, fontSizePt: number): number {
	if (!text) return 0;
	let width = 0;
	try {
		const Ctor = (Intl as unknown as { Segmenter?: SegmenterCtor }).Segmenter;
		if (Ctor) {
			const seg = new Ctor('en', { granularity: 'grapheme' });
			for (const { segment } of seg.segment(text)) {
				const code = segment.codePointAt(0) || 0;
				if (code >= 0x0e00 && code <= 0x0e7f) {
					width += fontSizePt * 0.55;
				} else if (code === 0x20) {
					width += fontSizePt * 0.25;
				} else if (code >= 0x30 && code <= 0x39) {
					width += fontSizePt * 0.55;
				} else {
					width += fontSizePt * 0.5;
				}
			}
			return width * 1.1;
		}
	} catch {
		/* fallthrough */
	}
	return text.length * fontSizePt * 0.55 * 1.1;
}

/** Wrap Thai text เป็นหลายบรรทัดที่ขอบเขตคำ ใส่ \n (hard break) ระหว่างบรรทัด
 *  คำในบรรทัดเดียวกันติดกันไม่มี space → ไม่มีเว้นวรรค + ไม่มี tofu
 *
 *  สำคัญ: เริ่มต้นด้วย split user's explicit \n (กิจกรรม batch อนุญาตให้ user
 *  ใส่ "\n" เอง) → preserve intent ของ user, แล้วค่อย wrap แต่ละ section
 *
 *  ใช้ Intl.Segmenter('th', word) ตัดคำตาม dictionary ของ ICU
 *  fallback: ถ้า word ใด ๆ กว้างกว่า maxWidth (คำทับศัพท์ที่ dict ไม่รู้จัก เช่น
 *  "ไฮโดรโปนิกส์") → sub-segment เป็น grapheme cluster (= syllable ในไทย)
 *  fallback อีกระดับ = return text เดิมถ้า Segmenter ไม่มี */
function wrapThaiToLines(
	text: string | undefined | null,
	maxWidthPt: number,
	fontSizePt: number
): string {
	if (!text) return '';
	// preserve user's explicit \n (batch activity titles) — ตัดก่อน แล้ว wrap แต่ละ section
	const sections = text.split(/\r?\n/);
	return sections.map((s) => wrapSection(s, maxWidthPt, fontSizePt)).join('\n');
}

function wrapSection(text: string, maxWidthPt: number, fontSizePt: number): string {
	if (!text) return '';
	try {
		const Ctor = (Intl as unknown as { Segmenter?: SegmenterCtor }).Segmenter;
		if (!Ctor) return text;
		const wordSeg = new Ctor('th', { granularity: 'word' });
		const words = Array.from(wordSeg.segment(text), (s) => s.segment).filter(
			(s) => s.length > 0
		);

		// ถ้า word ใดกว้างกว่า cell → ลอง syllable split (ตัดก่อน leading vowel)
		// fallback ถ้า syllable split ไม่ได้ → grapheme cluster
		const graphSeg = new Ctor('en', { granularity: 'grapheme' });
		const units: string[] = [];
		for (const w of words) {
			if (measureTextWidthPt(w, fontSizePt) > maxWidthPt) {
				const subs = syllableSplit(w);
				if (subs.length > 1) {
					for (const s of subs) units.push(s);
				} else {
					for (const g of graphSeg.segment(w)) units.push(g.segment);
				}
			} else {
				units.push(w);
			}
		}

		if (units.length <= 1) return text;

		const lines: string[] = [];
		let line = '';
		let lineWidth = 0;
		for (const unit of units) {
			const uw = measureTextWidthPt(unit, fontSizePt);
			if (line === '' || lineWidth + uw <= maxWidthPt) {
				line += unit;
				lineWidth += uw;
			} else {
				lines.push(line);
				line = unit;
				lineWidth = uw;
			}
		}
		if (line) lines.push(line);
		return lines.join('\n');
	} catch {
		return text;
	}
}

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

// Compact layout สำหรับ grid-2×2 mode — padding/border ลด 50%
const miniTableLayout: CustomTableLayout = {
	hLineWidth: () => 0.5,
	vLineWidth: () => 0.5,
	hLineColor: () => '#9ca3af',
	vLineColor: () => '#9ca3af',
	paddingLeft: () => 1,
	paddingRight: () => 1,
	paddingTop: () => 1,
	paddingBottom: () => 1,
	fillColor: () => null
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

/** อ่าน natural width/height ของรูป (data URL หรือ URL) ผ่าน Image element
 *  ใช้สำหรับคำนวณขนาด rendered (pdfmake's fit) แบบ exact → centering แม่นยำ */
async function getImageDimensions(
	dataUrl: string
): Promise<{ w: number; h: number } | null> {
	if (typeof Image === 'undefined') return null;
	return await new Promise((resolve) => {
		const img = new Image();
		img.onload = () => resolve({ w: img.naturalWidth, h: img.naturalHeight });
		img.onerror = () => resolve(null);
		img.src = dataUrl;
	});
}

function buildPageContent(
	page: TimetablePage,
	isFirst: boolean,
	logoDataUrl: string | null,
	logoDims: { w: number; h: number } | null
): Content[] {
	const { title, subTitle, periods, timetableEntries, viewMode = 'CLASSROOM', roomNames } = page;
	const tableBody: TableCell[][] = [];

	// คำนวณ width upfront เพื่อแชร์กับ text-wrap (ดู buildPageContent return)
	const N = periods.length + 1;
	const PAD_LR = 4; // 2+2 from tableLayout
	const BORDER = 1;
	const offsetsTotal = PAD_LR * N + BORDER * (N + 1);
	const pageContentWidth = 841.89 - 10 - 10;
	const safety = 2;
	const maxSumWidths = pageContentWidth - offsetsTotal - safety;
	const DAY_COL = 55; // กว้างขึ้นจาก 40 → 55 เพื่อให้ logo (rowSpan=3) มีที่พอ
	const periodWidth = (maxSumWidths - DAY_COL) / Math.max(1, periods.length);
	const cellContentWidth = periodWidth - PAD_LR; // padding eats periodWidth

	// QR code link — แสดงเฉพาะ INSTRUCTOR (ครูสแกนเปิดดูตารางสอนได้)
	// ใช้ window.location.origin เพื่อ work กับทุก school subdomain
	const showQrCode = viewMode === 'INSTRUCTOR' && typeof window !== 'undefined';
	const qrUrl = showQrCode ? window.location.origin + '/staff/timetable' : '';

	// === Row 0: Title row ===
	// logo (rowSpan=3) ครอบ title row + period name row + time row
	// title cell (colSpan=N) ใช้ inner columns เพื่อใส่ QR ที่มุมขวา
	// Logo cell — rowSpan=3 ครอบ title + period name + time rows
	// ใช้ NESTED TABLE ขนาด fixed + verticalAlignment 'middle' ใน inner cell
	// (cell-level VA ทำงานได้แม่นกว่า rowSpan VA ที่ buggy)
	//
	// nested table height = ประมาณการ rowSpan visual total:
	//   INSTRUCTOR: row 0 (~52, QR-driven) + row 1 (~22) + row 2 (~14) = ~87pt ✓ user OK
	//   CLASSROOM:  row 0 (~45, title only) + row 1 (~22) + row 2 (~14) = ~81pt
	//     (เดิม 67 → ครอบแค่ row 0+row 1 → ภาพชิดบน)
	const FIT_W = DAY_COL - 4; // 51
	const NESTED_H = showQrCode ? 87 : 81;
	const FIT_H = NESTED_H - 4;

	const logoCell: TableCell = logoDataUrl
		? ({
				table: {
					widths: ['*'],
					heights: [NESTED_H],
					body: [
						[
							{
								stack: [
									{ image: logoDataUrl, fit: [FIT_W, FIT_H], alignment: 'center' }
								],
								verticalAlignment: 'middle',
								alignment: 'center',
								border: [false, false, false, false]
							} as unknown as TableCell
						]
					]
				},
				layout: 'noBorders',
				rowSpan: 3,
				alignment: 'center'
			} as unknown as TableCell)
		: ({ text: '', rowSpan: 3 } as TableCell);

	const titleCellInner = {
		columns: [
			{
				stack: [
					{
						text: title,
						bold: true,
						fontSize: 16,
						color: '#1e3a8a',
						alignment: 'center',
						margin: [0, 6, 0, 0]
					},
					{
						text: subTitle,
						fontSize: 11,
						color: '#4b5563',
						alignment: 'center',
						margin: [0, 2, 0, 6]
					}
				],
				width: '*'
			},
			...(showQrCode
				? [
						{
							width: 65,
							stack: [
								// margin-top บน QR = caption height (~8pt) → QR image อยู่กึ่งกลาง stack แนวตั้ง
								// (stack เต็ม cell แนวตั้งอยู่แล้ว เพราะ QR เป็น content ที่สูงสุดใน row)
								{
									qr: qrUrl,
									fit: 55,
									alignment: 'center' as const,
									margin: [0, 8, 0, 0] as [number, number, number, number]
								},
								{
									text: 'สแกนดูตาราง',
									fontSize: 6,
									color: '#6b7280',
									alignment: 'center' as const,
									margin: [0, 1, 0, 0] as [number, number, number, number]
								}
							]
						}
					]
				: [])
		]
	};

	const titleRow: TableCell[] = [
		logoCell,
		{
			...titleCellInner,
			colSpan: periods.length,
			margin: [0, 0, 0, 0]
		} as unknown as TableCell,
		// ghost cells สำหรับ colSpan (periods.length - 1 cells)
		...Array(Math.max(0, periods.length - 1)).fill('')
	];
	tableBody.push(titleRow);

	// === Row 1: Period name row (logo ต่อจากแถวบน) ===
	const periodNameRow: TableCell[] = [''];
	periods.forEach((p) => {
		const labelText = p.name && p.name.trim() ? p.name : ' ';
		periodNameRow.push({
			text: labelText,
			bold: true,
			fontSize: 9,
			alignment: 'center',
			fillColor: '#f3f4f6',
			margin: [0, 2]
		});
	});
	tableBody.push(periodNameRow);

	// === Row 2: Time row (logo ต่อจากแถวบน) ===
	// fitTimeRange จัดให้พอดี cellContentWidth (1 บรรทัดก่อน, fallback 2 บรรทัด)
	const timeRow: TableCell[] = [''];
	periods.forEach((p) => {
		const fitted = fitTimeRange(p.start_time, p.end_time, cellContentWidth);
		timeRow.push({
			text: fitted.text,
			fontSize: fitted.fontSize,
			color: '#4b5563',
			alignment: 'center',
			fillColor: '#f3f4f6',
			margin: [0, 1]
		});
	});
	tableBody.push(timeRow);

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
							text: wrapThaiToLines(
								entry.subject_name_th || entry.subject_name_en || 'วิชา',
								cellContentWidth,
								7
							),
							fontSize: 7,
							margin: [0, 0]
						}
					);
				} else {
					stack.push({
						text: wrapThaiToLines(entry.title || 'กิจกรรม', cellContentWidth, 8),
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
						stack.push({
							text: wrapThaiToLines(teacherName, cellContentWidth, 7),
							fontSize: 7,
							color: '#4b5563',
							margin: [0, 1]
						});
					}
				} else {
					// Teacher PDF — แสดงห้อง (ยกเว้น sync activity เพราะเป็นกิจกรรมรวมทุกห้อง)
					if (!isSlotSync && entry.classroom_name) {
						stack.push({
							text: wrapThaiToLines(entry.classroom_name, cellContentWidth, 7),
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
						text: wrapThaiToLines(roomDisplay, cellContentWidth, 7),
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

	// ช่องลงชื่อใต้ตารางสอน — แสดงเฉพาะ INSTRUCTOR view (ตามที่ user ขอ)
	const signatureBlock: Content | null =
		viewMode === 'INSTRUCTOR'
			? {
					columns: [
						{
							stack: [
								{ text: 'ลงชื่อ ........................................', alignment: 'center' },
								{
									text: 'หัวหน้ากลุ่มบริหารงานวิชาการ',
									alignment: 'center',
									fontSize: 11,
									margin: [0, 4, 0, 0]
								}
							],
							width: '*'
						},
						{
							stack: [
								{ text: 'ลงชื่อ ........................................', alignment: 'center' },
								{
									text: 'ผู้อำนวยการ',
									alignment: 'center',
									fontSize: 11,
									margin: [0, 4, 0, 0]
								}
							],
							width: '*'
						}
					],
					margin: [0, 30, 0, 0]
				}
			: null;

	const tableContent: Content = {
		table: {
			// headerRows: 3 → title + period name + time จะ repeat ตอน page break
			// (ไม่เกิดในปกติเพราะ dontBreakRows + พอดี 1 หน้า แต่กันไว้)
			headerRows: 3,
			// Fixed widths — `widths` ของ pdfmake = CONTENT width per cell
			// tableWidth จริง = sum(widths) + offsetsTotal
			// offsetsTotal = (paddingL + paddingR) × N + vLineWidth × (N+1)
			// ดู: pdfmake/src/TableProcessor.js:100 + DocMeasure.js:596-614
			widths: [DAY_COL, ...periods.map(() => periodWidth)],
			heights: ['auto', 'auto', 'auto', 50, 50, 50, 50, 50],
			body: tableBody,
			dontBreakRows: true
		},
		layout: tableLayout,
		...(isFirst ? {} : { pageBreak: 'before' })
	} as Content;

	return [
		tableContent,
		...(signatureBlock ? [signatureBlock] : []),
		{
			columns: [
				{ text: `ข้อมูล ณ วันที่ ${new Date().toLocaleDateString('th-TH')}`, style: 'footer' },
				{ text: 'SchoolOrbit TimeTable', style: 'footer', alignment: 'right' }
			],
			margin: [0, 10, 0, 0]
		}
	];
}

/** Mini-table สำหรับ portrait-2col mode — แสดงข้อมูลครบ (วิชา ครู/ห้องเรียน ห้อง)
 *  font เล็ก + auto-wrap, row height ใหญ่ขึ้นเพื่อรองรับ multi-line
 *  miniAreaWidth: ความกว้างที่ allocated ต่อ 1 mini (default 400 — legacy,
 *                 282 สำหรับ portrait 2-col) */
function buildMiniTable(page: TimetablePage, miniAreaWidth: number = 400): Content {
	const { periods, timetableEntries, title, viewMode = 'CLASSROOM', roomNames } = page;
	const tableBody: TableCell[][] = [];

	// คำนวณ width upfront เพื่อแชร์กับ text-wrap
	const N = periods.length + 1;
	const PAD_LR = 2; // 1+1 from miniTableLayout
	const BORDER = 0.5;
	const offsetsTotal = PAD_LR * N + BORDER * (N + 1);
	const safety = 1;
	const maxSumWidths = miniAreaWidth - offsetsTotal - safety;
	const DAY_COL = 18;
	const periodWidth = (maxSumWidths - DAY_COL) / Math.max(1, periods.length);
	const cellContentWidth = periodWidth - PAD_LR; // padding eats periodWidth

	// Header row — period name + start time
	const headerRow: TableCell[] = [
		{ text: 'วัน', bold: true, alignment: 'center', fillColor: '#f3f4f6', fontSize: 5, margin: [0, 0] }
	];
	periods.forEach((p) => {
		const labelText = p.name && p.name.trim() ? p.name : ' ';
		headerRow.push({
			text: [
				{ text: `${labelText}\n`, bold: true, fontSize: 5 },
				{ text: formatTime(p.start_time), fontSize: 4, color: '#4b5563' }
			],
			alignment: 'center',
			fillColor: '#f3f4f6',
			margin: [0, 0]
		});
	});
	tableBody.push(headerRow);

	// Data rows — ย่อชื่อวันเหลือ 1 ตัวอักษร ("จ" / "อ" / "พ" / "พฤ" / "ศ")
	const dayShort: Record<string, string> = { MON: 'จ', TUE: 'อ', WED: 'พ', THU: 'พฤ', FRI: 'ศ' };
	DAYS.slice(0, 5).forEach((day) => {
		const row: TableCell[] = [
			{
				text: dayShort[day.value] || day.label.slice(0, 1),
				bold: true,
				alignment: 'center',
				fillColor: day.color,
				fontSize: 6,
				margin: [0, 0]
			}
		];

		periods.forEach((p) => {
			const entry = getEntry(timetableEntries, day.value, p.id);
			if (entry) {
				const stack: Content[] = [];
				if (entry.entry_type === 'COURSE') {
					stack.push({
						text: entry.subject_code || '',
						bold: true,
						fontSize: 4,
						color: '#1e3a8a'
					});
					const name = entry.subject_name_th || entry.subject_name_en;
					if (name) {
						stack.push({
							text: wrapThaiToLines(name, cellContentWidth, 3.5),
							fontSize: 3.5,
							color: '#374151',
							margin: [0, 0],
							lineHeight: 0.9
						});
					}
				} else {
					stack.push({
						text: wrapThaiToLines(entry.title || 'กิจกรรม', cellContentWidth, 4),
						bold: true,
						fontSize: 4,
						color: '#047857',
						lineHeight: 0.9
					});
				}

				// SLOT-sync activity (ครูหลายคน, ห้องหลายห้อง) — ซ่อน meta side
				const isSlotSync =
					entry.entry_type === 'ACTIVITY' && entry.activity_scheduling_mode === 'synchronized';

				if (viewMode === 'CLASSROOM') {
					if (
						!isSlotSync &&
						entry.instructor_name &&
						entry.instructor_name.trim() &&
						entry.instructor_name !== '-'
					) {
						const rawName = entry.instructor_name.trim();
						const teacherName = rawName.startsWith('ครู') ? rawName : `ครู${rawName}`;
						stack.push({
							text: wrapThaiToLines(teacherName, cellContentWidth, 3.5),
							fontSize: 3.5,
							color: '#4b5563',
							lineHeight: 0.9
						});
					}
				} else {
					if (!isSlotSync && entry.classroom_name) {
						stack.push({
							text: wrapThaiToLines(entry.classroom_name, cellContentWidth, 3.5),
							fontSize: 3.5,
							color: '#d97706',
							bold: true,
							lineHeight: 0.9
						});
					}
				}

				// ชื่อห้อง — ใช้ name_th ถ้ามี, fallback เป็น room_code
				const roomFullName = entry.room_id ? roomNames?.[entry.room_id] : undefined;
				const roomDisplay = roomFullName
					? roomFullName
					: entry.room_code
						? `ห้อง ${entry.room_code}`
						: null;
				if (roomDisplay) {
					stack.push({
						text: wrapThaiToLines(roomDisplay, cellContentWidth, 3.5),
						fontSize: 3.5,
						background: '#f3f4f6',
						color: '#1f2937',
						lineHeight: 0.9
					});
				}

				row.push({ stack, alignment: 'center', margin: [0, 0] });
			} else {
				row.push({ text: '' });
			}
		});

		tableBody.push(row);
	});

	// Widths array — uses precomputed periodWidth + DAY_COL from top of function
	const widths = [DAY_COL, ...periods.map(() => periodWidth)];

	// row height 38pt → รองรับ multi-line (code + name 2 lines + teacher 1-2 lines + room 1-2 lines)
	// cast as Content — pdfmake รับ width ใน column context แต่ TS type ไม่ครอบคลุม
	return {
		stack: [
			{
				text: stripTitlePrefix(title),
				fontSize: 8,
				bold: true,
				alignment: 'center',
				margin: [0, 0, 0, 2]
			},
			{
				table: {
					headerRows: 1,
					widths,
					heights: ['auto', 38, 38, 38, 38, 38],
					body: tableBody,
					dontBreakRows: true
				},
				layout: miniTableLayout
			}
		],
		width: '*'
	} as Content;
}

/** Portrait 2-column page — รวม mini-tables ใน 2 คอลัมเรียงลงมา (newspaper order)
 *  6 minis/page (3 rows × 2 cols) — ปรับให้พอดี A4 portrait */
const PORTRAIT_MINI_AREA_WIDTH = 282; // (595.28 - 20 margins - 10 gap) / 2 = 282.64 — เผื่อ 0.64
const MINIS_PER_PORTRAIT_PAGE = 6;

function buildPortraitPageContent(
	chunk: TimetablePage[],
	isFirst: boolean,
	logoDataUrl: string | null,
	pageHeaderTitle: string,
	pageHeaderSubTitle: string
): Content[] {
	// QR code — แสดงเฉพาะ INSTRUCTOR view (เหมือนกับ full mode)
	const isInstructor = chunk[0]?.viewMode === 'INSTRUCTOR';
	const showQr = isInstructor && typeof window !== 'undefined';
	const qrUrl = showQr ? window.location.origin + '/staff/timetable' : '';
	const qrCornerBlock = (
		showQr
			? {
					width: 70,
					stack: [
						{ qr: qrUrl, fit: 65, alignment: 'center' },
						{
							text: 'สแกนดูตาราง',
							fontSize: 6,
							color: '#6b7280',
							alignment: 'center',
							margin: [0, 1, 0, 0]
						}
					]
				}
			: { text: '', width: 40 }
	) as unknown as Content;

	const titleBlock: Content = logoDataUrl
		? {
				columns: [
					{
						width: 40,
						stack: [{ image: logoDataUrl, fit: [40, 40], alignment: 'center' }]
					},
					{
						stack: [
							{ text: pageHeaderTitle, fontSize: 14, bold: true, color: '#1e3a8a', alignment: 'center' },
							{
								text: pageHeaderSubTitle,
								fontSize: 10,
								color: '#4b5563',
								alignment: 'center',
								margin: [0, 2, 0, 0]
							}
						],
						width: '*'
					},
					qrCornerBlock
				],
				columnGap: 10,
				margin: [0, 0, 0, 8],
				...(isFirst ? {} : { pageBreak: 'before' })
			}
		: {
				columns: [
					{
						stack: [
							{ text: pageHeaderTitle, fontSize: 14, bold: true, color: '#1e3a8a', alignment: 'center', margin: [0, 0, 0, 2] },
							{ text: pageHeaderSubTitle, fontSize: 10, color: '#4b5563', alignment: 'center', margin: [0, 0, 0, 8] }
						],
						width: '*'
					},
					qrCornerBlock
				],
				columnGap: 10,
				...(isFirst ? {} : { pageBreak: 'before' })
			};

	// แต่ละ mini เพิ่ม bottom margin 8pt เพื่อ separation ใน stack
	const minis: Content[] = chunk.map((p) => {
		const mini = buildMiniTable(p, PORTRAIT_MINI_AREA_WIDTH);
		return { ...(mini as object), margin: [0, 0, 0, 8] } as Content;
	});

	// Newspaper order: index 0,2,4 → ซ้าย, 1,3,5 → ขวา
	const leftCol = minis.filter((_, i) => i % 2 === 0);
	const rightCol = minis.filter((_, i) => i % 2 === 1);

	return [
		titleBlock,
		{
			columns: [
				{ stack: leftCol, width: '*' },
				{ stack: rightCol, width: '*' }
			],
			columnGap: 10
		},
		{
			columns: [
				{ text: `ข้อมูล ณ วันที่ ${new Date().toLocaleDateString('th-TH')}`, style: 'footer' },
				{ text: 'SchoolOrbit TimeTable', style: 'footer', alignment: 'right' }
			],
			margin: [0, 8, 0, 0]
		}
	];
}

export interface GeneratePdfOptions {
	/** 'full' = 1 ตาราง/หน้า A4 landscape (default, รายละเอียดเต็ม)
	 *  'portrait-2col' = หลายตาราง/หน้า A4 portrait, 2 คอลัมเรียงลงมา (สำหรับเปรียบเทียบ) */
	layout?: 'full' | 'portrait-2col';
}

export const generateTimetablePDF = async (
	pages: TimetablePage[],
	fileName?: string,
	options?: GeneratePdfOptions
) => {
	if (pages.length === 0) return;
	const layout = options?.layout ?? 'full';

	// โหลด Sarabun สำหรับ canvas-based text measurement (wrap แม่นยำ)
	// + load pdfmake พร้อมกัน
	const [pdfMakeModule] = await Promise.all([
		import('pdfmake/build/pdfmake'),
		ensureSarabunMeasurement()
	]);
	const pdfMake = pdfMakeModule.default;

	pdfMake.fonts = {
		Sarabun: {
			normal: window.location.origin + '/fonts/Sarabun-Regular.ttf',
			bold: window.location.origin + '/fonts/Sarabun-Bold.ttf',
			italics: window.location.origin + '/fonts/Sarabun-Regular.ttf',
			bolditalics: window.location.origin + '/fonts/Sarabun-Bold.ttf'
		}
	};

	// Fetch โลโก้โรงเรียน (ถ้ามี) เป็น base64 data URL + อ่าน natural dimensions
	// dimensions ใช้คำนวณ rendered height ของ logo เพื่อ auto-center แนวตั้งใน rowSpan cell
	let logoDataUrl: string | null = null;
	let logoDims: { w: number; h: number } | null = null;
	try {
		const settings = await getSchoolSettings();
		if (settings.logoUrl) {
			logoDataUrl = await fetchImageDataUrl(settings.logoUrl);
			if (logoDataUrl) {
				logoDims = await getImageDimensions(logoDataUrl);
			}
		}
	} catch {
		/* ไม่มี logo ก็ไม่เป็นไร */
	}

	let content: Content[];
	if (layout === 'portrait-2col') {
		// chunk pages ละ MINIS_PER_PORTRAIT_PAGE → 1 PDF page = หลาย mini-tables
		const chunks: TimetablePage[][] = [];
		for (let i = 0; i < pages.length; i += MINIS_PER_PORTRAIT_PAGE) {
			chunks.push(pages.slice(i, i + MINIS_PER_PORTRAIT_PAGE));
		}
		// page header (1 ครั้ง/หน้า) — title generic ตาม viewMode, subTitle ใช้ของหน้าแรก
		const isClassroom = (pages[0].viewMode ?? 'CLASSROOM') === 'CLASSROOM';
		const pageHeaderTitle = isClassroom ? 'ตารางเรียน' : 'ตารางสอน';
		const pageHeaderSubTitle = pages[0].subTitle;
		content = chunks.flatMap((chunk, i) =>
			buildPortraitPageContent(chunk, i === 0, logoDataUrl, pageHeaderTitle, pageHeaderSubTitle)
		);
	} else {
		content = pages.flatMap((page, i) => buildPageContent(page, i === 0, logoDataUrl, logoDims));
	}

	const docDefinition: TDocumentDefinitions = {
		pageSize: 'A4',
		// portrait mode: 2-col stacked minis; landscape: 1 full-page table
		pageOrientation: layout === 'portrait-2col' ? 'portrait' : 'landscape',
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
