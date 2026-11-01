import { expect, type Download } from '@playwright/test';
import { PDFArray, PDFDocument, PDFRawStream, decodePDFRawStream } from 'pdf-lib';
import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { getDocument, OPS } from 'pdfjs-dist/legacy/build/pdf.mjs';

/** Verify the delivered PDF has four vector corner arcs per table on every page. */
export async function expectRoundedTimetablePdf(download: Download, tablesPerPage: number[]) {
	const pdf = await PDFDocument.load(readFileSync((await download.path())!));
	expect(pdf.getPageCount()).toBe(tablesPerPage.length);
	for (const [index, page] of pdf.getPages().entries()) {
		const contents = page.node.Contents();
		expect(contents).toBeDefined();
		const streams = contents instanceof PDFArray ? contents.asArray() : [contents!];
		const operators = streams
			.map((entry) => {
				const stream = pdf.context.lookup(entry);
				if (!(stream instanceof PDFRawStream)) throw new Error('Expected a PDF content stream');
				return Buffer.from(decodePDFRawStream(stream).decode()).toString();
			})
			.join('\n');
		const number = '-?\\d+(?:\\.\\d+)?';
		const arcs =
			operators.match(new RegExp(`(?:${number}\\s+){2}m\\s+(?:${number}\\s+){6}c\\s+S`, 'g')) ?? [];
		expect(arcs, `rounded corners on PDF page ${index + 1}`).toHaveLength(tablesPerPage[index] * 4);
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

/** Compare the drawn crest against its actual enclosing header border, preserving aspect. */
export async function expectCenteredTimetablePdfLogos(
	download: Download,
	aspect: number,
	counts: number[]
) {
	const { pdf } = await deliveredPdf(download);
	for (let index = 0; index < pdf.getPageCount(); index++) {
		const { images, segments } = geometry(
			pageOperators(pdf, index),
			pdf.getPages()[index].getHeight()
		);
		expect(images).toHaveLength(counts[index]);
		for (const image of images) {
			const cx = image.x + image.width / 2;
			const cy = image.y + image.height / 2;
			const horizontal = segments.filter(
				(line) =>
					line.y1 === line.y2 && Math.min(line.x1, line.x2) < cx && Math.max(line.x1, line.x2) > cx
			);
			const top = Math.max(...horizontal.filter((line) => line.y1 < cy).map((line) => line.y1));
			const bottom = Math.min(...horizontal.filter((line) => line.y1 > cy).map((line) => line.y1));
			const middle = (top + bottom) / 2;
			const vertical = segments.filter(
				(line) =>
					line.x1 === line.x2 &&
					Math.min(line.y1, line.y2) <= middle &&
					Math.max(line.y1, line.y2) >= middle
			);
			const left = Math.max(...vertical.filter((line) => line.x1 < cx).map((line) => line.x1));
			const right = Math.min(...vertical.filter((line) => line.x1 > cx).map((line) => line.x1));
			expect(Number.isFinite(top + bottom + left + right)).toBe(true);
			expect(Math.abs(cx - (left + right) / 2)).toBeLessThan(0.6);
			expect(Math.abs(cy - middle)).toBeLessThan(0.6);
			expect(image.width / image.height).toBeCloseTo(aspect, 3);
			expect(image.x).toBeGreaterThan(left);
			expect(image.y).toBeGreaterThan(top);
			expect(image.x + image.width).toBeLessThan(right);
			expect(image.y + image.height).toBeLessThan(bottom);
		}
	}
}
