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
