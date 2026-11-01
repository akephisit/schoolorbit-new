import { expect, type Download } from '@playwright/test';
import { PDFArray, PDFDocument, PDFRawStream, decodePDFRawStream } from 'pdf-lib';
import { readFileSync } from 'node:fs';

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

/** Check actual page operators: text is black and vector fills have no color tint.
 * Embedded crest images are separate XObjects and retain their original colors. */
export async function expectMonochromeTimetablePdf(download: Download) {
	const pdf = await PDFDocument.load(readFileSync((await download.path())!));
	for (const [index, page] of pdf.getPages().entries()) {
		const contents = page.node.Contents();
		expect(contents).toBeDefined();
		const streams = contents instanceof PDFArray ? contents.asArray() : [contents!];
		let fill = [0, 0, 0];
		const savedFills: number[][] = [];
		let textRuns = 0;
		for (const entry of streams) {
			const stream = pdf.context.lookup(entry);
			if (!(stream instanceof PDFRawStream)) throw new Error('Expected a PDF content stream');
			const operators = Buffer.from(decodePDFRawStream(stream).decode()).toString();
			for (const line of operators.split('\n').map((line) => line.trim())) {
				if (line === 'q') savedFills.push([...fill]);
				if (line === 'Q') fill = savedFills.pop() ?? [0, 0, 0];
				const rgb = line.match(/^([\d.]+)\s+([\d.]+)\s+([\d.]+)\s+(?:scn|rg)$/);
				const gray = line.match(/^([\d.]+)\s+g$/);
				if (rgb) {
					fill = rgb.slice(1, 4).map(Number);
					expect(fill[0], `neutral fill on PDF page ${index + 1}`).toBe(fill[1]);
					expect(fill[1], `neutral fill on PDF page ${index + 1}`).toBe(fill[2]);
				} else if (gray) {
					fill = Array(3).fill(Number(gray[1]));
				}
				if (/(?:^|\s)(?:Tj|TJ)$/.test(line)) {
					textRuns++;
					expect(fill, `black text on PDF page ${index + 1}`).toEqual([0, 0, 0]);
				}
			}
		}
		expect(textRuns, `text operators on PDF page ${index + 1}`).toBeGreaterThan(0);
	}
}
