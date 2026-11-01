import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import { stripTypeScriptTypes } from 'node:module';
import pdfmake from 'pdfmake';
import { PDFDocument } from 'pdf-lib';
import { getDocument } from 'pdfjs-dist/legacy/build/pdf.mjs';
import { createTimetablePdfCorners } from '../../src/lib/utils/timetable-pdf-corners.ts';
import { expectCenteredTimetablePdfLogos } from '../e2e/timetable-pdf-assertions.ts';
import { createHash } from 'node:crypto';
import * as fontkit from 'fontkit';

test('ships genuine unmodified TH Sarabun New styles with the original embedding exception', () => {
	const notice = readFileSync(
		new URL('../../static/fonts/THSarabunNew-LICENSE.txt', import.meta.url),
		'utf8'
	);
	for (const style of ['', '-Bold', '-Italic', '-BoldItalic']) {
		const bytes = readFileSync(
			new URL(`../../static/fonts/THSarabunNew${style}.ttf`, import.meta.url)
		);
		const font = fontkit.create(bytes);
		assert.equal(font.familyName, 'TH Sarabun New');
		assert.equal(font.postscriptName, `THSarabunNew${style}`);
		assert.ok(font.hasGlyphForCodePoint('ก'.codePointAt(0)));
		assert.ok(notice.includes(createHash('sha256').update(bytes).digest('hex')));
	}
	assert.match(notice, /either version 2 of the License, or \(at your option\) any later version/);
	assert.match(notice, /embed this font or unaltered portions/);
	assert.match(notice, /GNU GENERAL PUBLIC LICENSE/);
});

// Exercise the actual production table builders with pdfmake, rather than fabricated layout markers.
const sourceUrl = new URL('../../src/lib/utils/pdf.ts', import.meta.url);
let source = readFileSync(sourceUrl, 'utf8');
for (const specifier of [
	'./timetable-pdf-corners.ts',
	'./timetable-pdf-class-label.ts',
	'../academic/timetable/teacher-label.ts',
	'#lib/utils/timetable-pdf-days.js',
	'#lib/utils/timetable-pdf-logo.js'
]) {
	const path = specifier.startsWith('#lib/')
		? specifier.replace('#lib/', '../').replace(/\.js$/, '.ts')
		: specifier;
	source = source.replace(specifier, new URL(path, sourceUrl).href);
}
// Branding delivery is a browser/API boundary covered by the download browser cases.
for (const [specifier, name] of [
	['#lib/api/school.js', 'getRequiredPublicSchoolInfo'],
	['#lib/api/files.js', 'downloadPublicFile']
]) {
	source = source.replace(
		specifier,
		`data:text/javascript;base64,${Buffer.from(`export function ${name}(){throw new Error('Browser API boundary')}`).toString('base64')}`
	);
}
const builders = await import(
	`data:text/javascript;base64,${Buffer.from(
		stripTypeScriptTypes(source + '\nexport { buildPageContent, buildPortraitPageContent };', {
			mode: 'transform'
		})
	).toString('base64')}`
);
const fonts = Object.fromEntries(
	Object.entries({ normal: '', bold: '-Bold', italics: '-Italic', bolditalics: '-BoldItalic' }).map(
		([style, suffix]) => [
			style,
			fileURLToPath(new URL(`../../static/fonts/THSarabunNew${suffix}.ttf`, import.meta.url))
		]
	)
);
pdfmake.addFonts({ THSarabunNew: fonts });

function pageFixture() {
	return {
		headerContext: 'ภาคเรียนที่ 1 ปีการศึกษา 2569',
		headerOwnerLabel: 'นายพิสิษฐ สกุลทดสอบ',
		viewMode: 'INSTRUCTOR',
		periods: Array.from({ length: 10 }, (_, i) => ({
			id: `period-${i}`,
			order_index: i,
			name: `คาบที่ ${i + 1}`,
			start_time: '08:00',
			end_time: '09:00'
		})),
		timetableBlocks: []
	};
}

async function renderTimetable(pages, layout, logo) {
	const corners = createTimetablePdfCorners();
	const content =
		layout === 'full'
			? pages.flatMap((page, i) =>
					builders.buildPageContent(page, i === 0, logo, 'โรงเรียนทดสอบ', corners)
				)
			: builders.buildPortraitPageContent(
					pages,
					true,
					logo,
					'โรงเรียนทดสอบ',
					'โรงเรียนทดสอบ',
					'ตารางสอนภาคเรียนที่ 1 ปีการศึกษา 2569',
					corners
				);
	return pdfmake
		.createPdf({
			pageSize: 'A4',
			pageOrientation: layout === 'full' ? 'landscape' : 'portrait',
			pageMargins: [10, 30, 10, 20],
			content,
			pageBreakBefore: corners.capture,
			defaultStyle: { font: 'THSarabunNew' }
		})
		.getBuffer();
}

const tallCrest =
	'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAADAAAACgAQMAAABwqn2HAAAAIGNIUk0AAHomAACAhAAA+gAAAIDoAAB1MAAA6mAAADqYAAAXcJy6UTwAAAAGUExURcIdPf///2PIUy0AAAABYktHRAH/Ai3eAAAAB3RJTUUH6gsBAhcx476U8gAAABJJREFUOMtjYBgFo2AUjALaAAAEYAAB7ZLQTgAAACV0RVh0ZGF0ZTpjcmVhdGUAMjAyNi0xMS0wMVQwMjoyMzo0OSswMDowMKBV6l0AAAAldEVYdGRhdGU6bW9kaWZ5ADIwMjYtMTEtMDFUMDI6MjM6NDkrMDA6MDDRCFLhAAAAKHRFWHRkYXRlOnRpbWVzdGFtcAAyMDI2LTExLTAxVDAyOjIzOjQ5KzAwOjAwhh1zPgAAAABJRU5ErkJggg==';

const wideCrest =
	'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAKAAAAAwAQMAAACsZ+MtAAAAIGNIUk0AAHomAACAhAAA+gAAAIDoAAB1MAAA6mAAADqYAAAXcJy6UTwAAAAGUExURcIdPf///2PIUy0AAAABYktHRAH/Ai3eAAAAB3RJTUUH6gsBAhcx476U8gAAABFJREFUKM9jYBgFo2AUjBQAAAPwAAHjcHIzAAAAJXRFWHRkYXRlOmNyZWF0ZQAyMDI2LTExLTAxVDAyOjIzOjQ5KzAwOjAwoFXqXQAAACV0RVh0ZGF0ZTptb2RpZnkAMjAyNi0xMS0wMVQwMjoyMzo0OSswMDowMNEIUuEAAAAodEVYdGRhdGU6dGltZXN0YW1wADIwMjYtMTEtMDFUMDI6MjM6NDkrMDA6MDCGHXM+AAAAAElFTkSuQmCC';

for (const [shape, logo, aspect] of [
	['tall', tallCrest, 48 / 160],
	['wide', wideCrest, 160 / 48]
]) {
	for (const [layout, count] of [
		['full', 1],
		['portrait-2col', 6]
	]) {
		test(`actual ${layout} PDF centers a ${shape} crest in its enclosing three-row header`, async () => {
			const bytes = await renderTimetable(Array.from({ length: count }, pageFixture), layout, logo);
			const pdf = await PDFDocument.load(bytes);
			assert.equal(pdf.getPageCount(), 1);
			const directory = mkdtempSync(join(tmpdir(), 'timetable-pdf-layout-'));
			try {
				const path = join(directory, 'output.pdf');
				writeFileSync(path, bytes);
				await expectCenteredTimetablePdfLogos({ path: async () => path }, aspect, [count]);
			} finally {
				rmSync(directory, { recursive: true, force: true });
			}
		});
	}
}

test('an oversized timetable retains the final day and its content across page breaks', async () => {
	const page = pageFixture();
	const block = {
		blockKind: 'course',
		isActive: true,
		bellSchedulePeriodId: 'period-0',
		groups: [],
		homerooms: [],
		teachers: []
	};
	page.timetableBlocks = [
		{
			...block,
			dayOfWeek: 'MON',
			offeringName: 'รายละเอียดบทเรียนหลายบรรทัด '.repeat(120),
			offeringCode: 'FIRST_DAY'
		},
		{ ...block, dayOfWeek: 'FRI', offeringCode: 'FINAL_ROW_SURVIVES', offeringName: 'วิชาสุดท้าย' }
	];
	const bytes = await renderTimetable([page], 'full', tallCrest);
	const loading = getDocument({ data: new Uint8Array(bytes), useSystemFonts: false });
	const pdf = await loading.promise;
	assert.ok(pdf.numPages > 1);
	let text = '';
	for (let i = 1; i <= pdf.numPages; i++) {
		const content = await (await pdf.getPage(i)).getTextContent();
		text += content.items.map((item) => ('str' in item ? item.str : '')).join('');
	}
	assert.ok(text.replace(/\s/g, '').includes('FINAL_ROW_SURVIVES'));
	assert.ok(text.includes('ศุกร์'));
	await loading.destroy();
});
