import { expect, type Download } from '@playwright/test';
import { PDFArray, PDFDocument, PDFRawStream, decodePDFRawStream } from 'pdf-lib';
import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { getDocument, OPS } from 'pdfjs-dist/legacy/build/pdf.mjs';

/** Verify square grids, complete border intersections and the expected tables on every page. */
export async function expectSquareTimetablePdf(download: Download, tablesPerPage: number[]) {
	const { pdf, rendered } = await deliveredPdf(download);
	expect(pdf.getPageCount()).toBe(tablesPerPage.length);
	for (const [index, page] of pdf.getPages().entries()) {
		const operators = pageOperators(pdf, index);
		expect(operators).not.toMatch(/(?:-?\d+(?:\.\d+)?\s+){6}c(?:\s|$)/);
		const { segments, rectangles } = geometry(operators, page.getHeight());
		const horizontal = segments.filter((line) => line.y1 === line.y2 && line.x1 !== line.x2);
		expect(horizontal.length).toBeGreaterThan(0);
		for (const line of horizontal) {
			for (const x of [line.x1, line.x2]) {
				expect(
					segments.some(
						(edge) =>
							Math.abs(edge.x1 - x) <= 0.501 &&
							edge.x2 === edge.x1 &&
							Math.min(edge.y1, edge.y2) <= line.y1 + 0.001 &&
							Math.max(edge.y1, edge.y2) >= line.y1 - 0.001
					),
					'square border intersection'
				).toBe(true);
			}
		}
		const text = await (await rendered.getPage(index + 1)).getTextContent();
		const firstGridTop = Math.min(...horizontal.map((line) => line.y1));
		const qrSquares = rectangles.filter(
			(rect) => rect.y < firstGridTop && rect.color.every((channel) => channel === 0)
		);
		const captions = text.items.filter((item) => 'str' in item && item.str.startsWith('สแกนดู'));
		if (qrSquares.length || captions.length) {
			expect(qrSquares.length, 'QR fill rectangles are present').toBeGreaterThan(0);
			expect(captions.length, 'QR has a complete caption').toBeGreaterThan(0);
			const qrBounds = {
				left: Math.min(...qrSquares.map((rect) => rect.x)),
				right: Math.max(...qrSquares.map((rect) => rect.x + rect.width)),
				top: Math.min(...qrSquares.map((rect) => rect.y)),
				bottom: Math.max(...qrSquares.map((rect) => rect.y + rect.height))
			};
			expect(qrBounds.left, 'QR left margin').toBeGreaterThanOrEqual(10 - 0.01);
			expect(qrBounds.right, 'QR right margin').toBeLessThanOrEqual(page.getWidth() - 10 + 0.01);
			expect(qrBounds.top, 'QR top margin').toBeGreaterThanOrEqual(30 - 0.01);
			expect(qrBounds.bottom, 'QR bottom margin').toBeLessThanOrEqual(page.getHeight() - 20 + 0.01);
			for (const item of captions) {
				if (!('str' in item)) continue;
				expect(item.str, 'caption must not be clipped at the page edge').toBe('สแกนดูตาราง');
				expect(item.transform[4], 'caption left margin').toBeGreaterThanOrEqual(10 - 0.01);
				expect(item.transform[4] + item.width, 'caption right margin').toBeLessThanOrEqual(
					page.getWidth() - 10 + 0.01
				);
				const top = page.getHeight() - item.transform[5] - item.height * 0.844;
				const bottom = page.getHeight() - item.transform[5] + item.height * 0.457;
				expect(top, 'caption top margin').toBeGreaterThanOrEqual(30 - 0.01);
				expect(bottom, 'caption bottom margin').toBeLessThanOrEqual(page.getHeight() - 20 + 0.01);
			}
		}
		const firstPeriods = text.items.filter(
			(item) => 'str' in item && /^คาบ(?:ที่)?\s*1$/.test(item.str.trim())
		);
		expect(firstPeriods, 'one period-name grid start per timetable').toHaveLength(
			tablesPerPage[index]
		);
		// These fixtures have single-line period names/ranges; a blank default12pt cell must not enlarge them.
		const full = page.getWidth() > page.getHeight();
		for (const [label, size, margins] of [
			['คาบ', full ? 9 : 7, full ? 4 : 0],
			['เวลา', full ? 10 : 6, full ? 2 : 0]
		] as const) {
			const labels = text.items.filter((item) => 'str' in item && item.str === label);
			expect(labels, `one ${label} heading per timetable`).toHaveLength(tablesPerPage[index]);
			for (const item of labels) {
				if (!('str' in item)) continue;
				expect(item.height).toBeCloseTo(size, 3);
				const cx = item.transform[4] + item.width / 2;
				const cy = page.getHeight() - item.transform[5] - item.height / 4;
				const boundaries = horizontal.filter(
					(line) => Math.min(line.x1, line.x2) < cx && Math.max(line.x1, line.x2) > cx
				);
				const top = Math.max(...boundaries.filter((line) => line.y1 < cy).map((line) => line.y1));
				const bottom = Math.min(
					...boundaries.filter((line) => line.y1 > cy).map((line) => line.y1)
				);
				const singleLineHeight = size * 1.301 + margins + (full ? 5 : 2.5);
				expect(bottom - top, `${label} row fits one Sarabun line plus its padding`).toBeCloseTo(
					singleLineHeight,
					3
				);
			}
		}

		// PDFJS fontName identifies the actual embedded glyph resource, independent of its size.
		const textItems = text.items.filter((item) => 'str' in item);
		const boldFonts = new Set(
			textItems.filter((item) => item.str === 'คาบ').map((item) => item.fontName)
		);
		const regularFonts = new Set(
			textItems.filter((item) => /^ตาราง(?:เรียน|สอน)/.test(item.str)).map((item) => item.fontName)
		);
		const timeLabelFonts = new Set(
			textItems.filter((item) => item.str === 'เวลา').map((item) => item.fontName)
		);
		const timeValueFonts = new Set(
			textItems.filter((item) => /^\d{2}:\d{2}/.test(item.str)).map((item) => item.fontName)
		);
		expect(
			[...timeLabelFonts].every((font) => boldFonts.has(font)),
			'เวลา uses the bold period-heading resource'
		).toBe(true);
		expect(timeValueFonts.size, 'time values are present').toBeGreaterThan(0);
		expect(
			[...timeValueFonts].every((font) => regularFonts.has(font) && !boldFonts.has(font)),
			'time values retain the regular context resource'
		).toBe(true);

		const fontSize = page.getWidth() > page.getHeight() ? 16 : 9;
		const schools = text.items.filter(
			(item) =>
				'str' in item && item.str.startsWith('โรงเรียน') && Math.abs(item.height - fontSize) < 0.01
		);
		for (const school of schools) {
			if (!('str' in school)) continue;
			const top = page.getHeight() - school.transform[5] - fontSize * 0.844;
			const bottom = page.getHeight() - school.transform[5] + fontSize * 0.457;
			const left =
				fontSize === 16 || school.transform[4] < page.getWidth() / 2 ? 10 : page.getWidth() / 2 + 5;
			const right =
				fontSize === 16 || school.transform[4] > page.getWidth() / 2
					? page.getWidth() - 10
					: page.getWidth() / 2 - 5;
			expect(
				segments.filter(
					(line) =>
						Math.max(line.x1, line.x2) > left &&
						Math.min(line.x1, line.x2) < right &&
						Math.max(line.y1, line.y2) > top + 0.01 &&
						Math.min(line.y1, line.y2) < bottom - 0.01
				),
				'header is outside the grid even without a logo'
			).toHaveLength(0);
		}
	}
}

interface Rectangle {
	x: number;
	y: number;
	width: number;
	height: number;
	color: number[];
}
interface Segment {
	x1: number;
	y1: number;
	x2: number;
	y2: number;
}
type Matrix = [number, number, number, number, number, number];
const identity: Matrix = [1, 0, 0, 1, 0, 0];
function multiply(a: Matrix, b: Matrix): Matrix {
	return [
		a[0] * b[0] + a[2] * b[1],
		a[1] * b[0] + a[3] * b[1],
		a[0] * b[2] + a[2] * b[3],
		a[1] * b[2] + a[3] * b[3],
		a[0] * b[4] + a[2] * b[5] + a[4],
		a[1] * b[4] + a[3] * b[5] + a[5]
	];
}

async function deliveredPdf(download: Download) {
	const path = (await download.path())!;
	const bytes = readFileSync(path);
	const pdf = await PDFDocument.load(bytes);
	const rendered = await getDocument({ data: new Uint8Array(bytes), useSystemFonts: false })
		.promise;
	return { pdf, rendered, path };
}
function pageOperators(pdf: PDFDocument, index: number): string {
	const contents = pdf.getPages()[index].node.Contents();
	expect(contents).toBeDefined();
	const streams = contents instanceof PDFArray ? contents.asArray() : [contents!];
	return streams
		.map((entry) => {
			const stream = pdf.context.lookup(entry);
			if (!(stream instanceof PDFRawStream)) throw new Error('Expected a PDF content stream');
			return Buffer.from(decodePDFRawStream(stream).decode()).toString();
		})
		.join('\n');
}
function geometry(operators: string, pageHeight: number) {
	let fill = [0, 0, 0];
	let matrix = identity;
	const stack: Array<{ fill: number[]; matrix: Matrix }> = [];
	const rectangles: Rectangle[] = [];
	const segments: Segment[] = [];
	const images: Array<{ x: number; y: number; width: number; height: number }> = [];
	let rectangle: Omit<Rectangle, 'color'> | undefined;
	let point: { x: number; y: number } | undefined;
	for (const line of operators.split('\n').map((line) => line.trim())) {
		const values = line.split(/\s+/);
		const command = values.pop();
		const n = values.map(Number);
		if (command === 'q') stack.push({ fill: [...fill], matrix: [...matrix] });
		if (command === 'Q') {
			const previous = stack.pop();
			if (previous) {
				fill = previous.fill;
				matrix = previous.matrix;
			}
		}
		if (command === 'cm' && n.length === 6) matrix = multiply(matrix, n as Matrix);
		if (command === 'scn' || command === 'rg') fill = n;
		if (command === 'g') fill = [n[0], n[0], n[0]];
		if (command === 're') rectangle = { x: n[0], y: n[1], width: n[2], height: n[3] };
		if (command === 'f' && rectangle) {
			rectangles.push({ ...rectangle, color: fill });
			rectangle = undefined;
		}
		if (command === 'm') point = { x: n[0], y: n[1] };
		if (command === 'l' && point) {
			segments.push({ x1: point.x, y1: point.y, x2: n[0], y2: n[1] });
			point = { x: n[0], y: n[1] };
		}
		if (command === 'Do' && /^\/I\d+$/.test(values[0])) {
			images.push({
				x: matrix[4],
				y: pageHeight - matrix[5] - matrix[3],
				width: Math.abs(matrix[0]),
				height: Math.abs(matrix[3])
			});
		}
	}
	return { rectangles, segments, images };
}

/** Assert delivered font bytes, equal rendered header sizes and actual white day cells. */
export async function expectTimetablePdfPresentation(
	download: Download,
	expectedDayLabels: readonly string[]
) {
	const { pdf, rendered, path } = await deliveredPdf(download);
	const fonts = execFileSync('pdffonts', [path], { encoding: 'utf8' }).trim().split('\n').slice(2);
	expect(fonts.length).toBeGreaterThanOrEqual(2);
	for (const font of fonts) {
		expect(font).toMatch(/^[A-Z]+\+THSarabunNew(?:-Bold)?\s/);
		expect(font).toMatch(/yes\s+yes\s+yes/);
	}
	const dayNames = new Set([
		'จันทร์',
		'อังคาร',
		'พุธ',
		'พฤหัสฯ',
		'ศุกร์',
		'เสาร์',
		'อาทิตย์',
		'จ',
		'อ',
		'พ',
		'พฤ',
		'ศ',
		'ส',
		'อา'
	]);
	for (let index = 0; index < pdf.getPageCount(); index++) {
		const page = await rendered.getPage(index + 1);
		const content = await page.getTextContent();
		const items = content.items.filter((item) => 'str' in item && item.str.trim());
		const { rectangles } = geometry(pageOperators(pdf, index), pdf.getPages()[index].getHeight());
		const days = items.filter((item) => 'str' in item && dayNames.has(item.str.trim()));
		const landscape = pdf.getPages()[index].getWidth() > pdf.getPages()[index].getHeight();
		const shortDays: Record<string, string> = {
			จันทร์: 'จ',
			อังคาร: 'อ',
			พุธ: 'พ',
			พฤหัสฯ: 'พฤ',
			ศุกร์: 'ศ',
			เสาร์: 'ส',
			อาทิตย์: 'อา'
		};
		const expectedLabels = expectedDayLabels.map((day) => (landscape ? day : shortDays[day]));
		const schoolCount = items.filter(
			(item) => 'str' in item && item.str.startsWith('โรงเรียน')
		).length;
		const tableCount = landscape ? schoolCount : schoolCount - 1;
		expect(tableCount).toBeGreaterThan(0);
		expect(days).toHaveLength(expectedLabels.length * tableCount);
		expect([...new Set(days.map((item) => ('str' in item ? item.str.trim() : '')))].sort()).toEqual(
			[...expectedLabels].sort()
		);
		for (const item of days) {
			if (!('str' in item)) continue;
			const x = item.transform[4] + item.width / 2;
			const y = pdf.getPages()[index].getHeight() - item.transform[5] - item.height / 4;
			const cell = rectangles.find(
				(rect) => x > rect.x && x < rect.x + rect.width && y > rect.y && y < rect.y + rect.height
			);
			expect(cell, `painted day cell for ${item.str}`).toBeDefined();
			expect(cell!.color, `white day cell for ${item.str}`).toEqual([1, 1, 1]);
		}
		const headers = items.filter(
			(item) =>
				'str' in item &&
				/^(โรงเรียน|ตาราง(?:เรียน|สอน)ภาคเรียน|นาย|นาง|ระดับชั้น|ม\.1\/1 คณิตศาสตร์)/.test(
					item.str.trim()
				)
		);
		expect(headers.length).toBeGreaterThanOrEqual(3);
		const schoolYs = headers
			.filter((item) => 'str' in item && item.str.startsWith('โรงเรียน'))
			.map((item) => ('str' in item ? item.transform[5] : 0));
		const outerSchoolY = Math.max(...schoolYs);
		const firstMiniSchoolY = Math.max(...schoolYs.filter((y) => y < outerSchoolY - 1));
		for (const item of headers) {
			if (!('str' in item)) continue;
			// Compact page header is16pt; every three-line mini header is9pt.
			const expected =
				pdf.getPages()[index].getWidth() > pdf.getPages()[index].getHeight()
					? 16
					: item.transform[5] > firstMiniSchoolY + 0.01
						? 16
						: 9;
			expect(item.height, `header font size: ${item.str}`).toBeCloseTo(expected, 3);
		}
	}
}

/** Match semantic text against the actual PDF fill color used to paint its glyphs. */
export async function expectTimetablePdfTextColors(
	download: Download,
	expected: Record<string, string>
) {
	const { pdf, rendered } = await deliveredPdf(download);
	const textByColor = new Map<string, string>();
	for (let i = 1; i <= pdf.getPageCount(); i++) {
		const operators = await (await rendered.getPage(i)).getOperatorList();
		let color = '#000000';
		const saved: string[] = [];
		for (let index = 0; index < operators.fnArray.length; index++) {
			const op = operators.fnArray[index];
			const args = operators.argsArray[index];
			if (op === OPS.save) saved.push(color);
			if (op === OPS.restore) color = saved.pop() ?? '#000000';
			if (op === OPS.setFillRGBColor) color = args[0];
			if (op === OPS.showText) {
				const glyphs = args[0] as Array<number | { unicode: string }>;
				const text = glyphs
					.filter((glyph) => typeof glyph !== 'number')
					.map((glyph) => (typeof glyph === 'number' ? '' : glyph.unicode))
					.join('');
				textByColor.set(color, (textByColor.get(color) ?? '') + text.replace(/\s/g, ''));
			}
		}
	}
	for (const [text, color] of Object.entries(expected))
		expect(textByColor.get(color) ?? '', `PDF text color ${color}: ${text}`).toContain(
			text.replace(/\s/g, '')
		);
}

/** Verify the external crest matches the rendered header line boxes above the square grid. */
export async function expectExternalTimetablePdfHeaders(
	download: Download,
	aspect: number,
	counts: number[]
) {
	const { pdf, rendered } = await deliveredPdf(download);
	expect(pdf.getPageCount()).toBe(counts.length);
	for (let index = 0; index < pdf.getPageCount(); index++) {
		const page = pdf.getPages()[index];
		const { images, segments } = geometry(pageOperators(pdf, index), page.getHeight());
		const content = await (await rendered.getPage(index + 1)).getTextContent();
		const fontSize = page.getWidth() > page.getHeight() ? 16 : 9;
		expect(images).toHaveLength(counts[index]);
		for (const image of images) {
			const areaRight =
				fontSize === 16 || image.x > page.getWidth() / 2
					? page.getWidth() - 10
					: page.getWidth() / 2 - 5;
			const header = content.items.filter((item) => {
				if (!('str' in item) || !item.str.trim() || Math.abs(item.height - fontSize) > 0.01)
					return false;
				const top = page.getHeight() - item.transform[5] - fontSize * 0.844;
				return (
					item.transform[4] >= image.x + image.width - 0.01 &&
					item.transform[4] + item.width <= areaRight + 0.01 &&
					top >= image.y - 0.01 &&
					top < image.y + image.height - 0.01
				);
			});
			expect(header.length).toBeGreaterThanOrEqual(3);
			const tops = header.map((item) =>
				'str' in item ? page.getHeight() - item.transform[5] - fontSize * 0.844 : 0
			);
			const bottoms = header.map((item) =>
				'str' in item ? page.getHeight() - item.transform[5] + fontSize * 0.457 : 0
			);
			const top = Math.min(...tops),
				bottom = Math.max(...bottoms);
			expect(Math.abs(image.y - top), 'crest starts at the school header line').toBeLessThan(0.01);
			expect(
				Math.abs(image.height - (bottom - top)),
				'crest height matches actual header line boxes'
			).toBeLessThan(0.01);
			expect(image.width / image.height).toBeCloseTo(aspect, 3);
			expect(image.y).toBeGreaterThanOrEqual(30 - 0.01);
			expect(image.y + image.height).toBeLessThan(page.getHeight() - 20);
			const bordersInHeader = segments.filter(
				(line) =>
					Math.max(line.x1, line.x2) > image.x + 0.01 &&
					Math.min(line.x1, line.x2) < areaRight &&
					Math.max(line.y1, line.y2) > top + 0.01 &&
					Math.min(line.y1, line.y2) < bottom - 0.01
			);
			expect(bordersInHeader, 'school, owner and crest are outside every grid border').toHaveLength(
				0
			);
			const gridTop = Math.min(
				...segments
					.filter(
						(line) =>
							line.y1 === line.y2 &&
							line.y1 > bottom &&
							Math.min(line.x1, line.x2) <= image.x + image.width / 2 &&
							Math.max(line.x1, line.x2) >= image.x + image.width / 2
					)
					.map((line) => line.y1)
			);
			expect(Number.isFinite(gridTop), 'header stays with its following grid').toBe(true);
			expect(gridTop - bottom).toBeGreaterThanOrEqual(fontSize === 16 ? 6 : 3);
			expect(gridTop - bottom).toBeLessThan(20);
			const runs = segments
				.filter((line) => line.y1 === gridTop && line.y2 === gridTop)
				.map((line) => ({ left: Math.min(line.x1, line.x2), right: Math.max(line.x1, line.x2) }))
				.sort((a, b) => a.left - b.left);
			const gridWidths: Array<{ left: number; right: number }> = [];
			for (const run of runs) {
				const previous = gridWidths.at(-1);
				if (previous && run.left <= previous.right + 0.001)
					previous.right = Math.max(previous.right, run.right);
				else gridWidths.push({ ...run });
			}
			const width = gridWidths.find(
				(run) => run.left < image.x + image.width / 2 && run.right > image.x + image.width / 2
			);
			expect(width).toBeDefined();
			expect(image.x - width!.left, 'crest is inset from its timetable leading edge').toBeCloseTo(
				fontSize === 16 ? 12 : 4,
				3
			);
		}
	}
}
