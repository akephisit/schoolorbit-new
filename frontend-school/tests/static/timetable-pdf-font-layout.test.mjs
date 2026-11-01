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
import {
	expectExternalTimetablePdfHeaders,
	expectSquareTimetablePdf
} from '../e2e/timetable-pdf-assertions.ts';
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
		assert.equal((font.ascent - font.descent) / font.unitsPerEm, 1.301);
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
		stripTypeScriptTypes(
			source +
				'\nexport { buildPageContent, buildMiniTable, buildPortraitPageContent, keepTimetableHeaderWithGrid };',
			{
				mode: 'transform'
			}
		)
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
		// A three-line fixture even under the deliberately conservative no-DOM width fallback.
		headerContext: 'ภาคเรียนที่ 1/2569',
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

async function renderTimetable(
	pages,
	layout,
	logoDataUrl,
	aspect = 48 / 160,
	schoolName = 'โรงเรียนทดสอบ'
) {
	const logo = logoDataUrl ? { dataUrl: logoDataUrl, aspectRatio: aspect } : null;
	const content =
		layout === 'full'
			? pages.flatMap((page, i) => builders.buildPageContent(page, i === 0, logo, schoolName, i))
			: Array.from({ length: Math.ceil(pages.length / 6) }, (_, i) =>
					builders.buildPortraitPageContent(
						pages.slice(i * 6, i * 6 + 6),
						i === 0,
						logo,
						schoolName,
						schoolName,
						'ตารางสอนภาคเรียนที่ 1 ปีการศึกษา 2569',
						i
					)
				).flat();
	return pdfmake
		.createPdf({
			pageSize: 'A4',
			pageOrientation: layout === 'full' ? 'landscape' : 'portrait',
			pageMargins: [10, 30, 10, 20],
			content,
			pageBreakBefore: builders.keepTimetableHeaderWithGrid,
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
		test(`actual ${layout} PDF places a ${shape} crest beside its external three-line header`, async () => {
			const bytes = await renderTimetable(
				Array.from({ length: count }, pageFixture),
				layout,
				logo,
				aspect
			);
			writeFileSync(`/tmp/timetable-external-${shape}-${layout}.pdf`, bytes);
			const pdf = await PDFDocument.load(bytes);
			assert.equal(pdf.getPageCount(), 1);
			const directory = mkdtempSync(join(tmpdir(), 'timetable-pdf-layout-'));
			try {
				const path = join(directory, 'output.pdf');
				writeFileSync(path, bytes);
				await expectExternalTimetablePdfHeaders({ path: async () => path }, aspect, [count]);
				await expectSquareTimetablePdf({ path: async () => path }, [count]);
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

async function withPdfArtifact(bytes, name, verify) {
	const path = join(tmpdir(), `timetable-external-${name}.pdf`);
	writeFileSync(path, bytes);
	await verify({ path: async () => path });
}

for (const layout of ['full', 'portrait-2col']) {
	test(`${layout} wraps long school names beside a proportional crest without a header border`, async () => {
		const school = 'โรงเรียนทดสอบวิทยาศาสตร์และเทคโนโลยีเพื่อการเรียนรู้ตลอดชีวิต '
			.repeat(3)
			.trim();
		const bytes = await renderTimetable([pageFixture()], layout, tallCrest, 48 / 160, school);
		await withPdfArtifact(bytes, `long-name-${layout}`, async (download) => {
			await expectExternalTimetablePdfHeaders(download, 48 / 160, [1]);
			await expectSquareTimetablePdf(download, [1]);
		});
		const loading = getDocument({ data: new Uint8Array(bytes), useSystemFonts: false });
		const content = await (await (await loading.promise).getPage(1)).getTextContent();
		const text = content.items
			.map((item) => ('str' in item ? item.str : ''))
			.join('')
			.replace(/\s/g, '');
		assert.ok(text.includes(school.replace(/\s/g, '')));
		await loading.destroy();
	});
	test(`${layout} preserves square grids and all school/owner text with no configured logo`, async () => {
		const bytes = await renderTimetable([pageFixture()], layout, null);
		await withPdfArtifact(bytes, `no-logo-${layout}`, async (download) => {
			await expectExternalTimetablePdfHeaders(download, 1, [0]);
			await expectSquareTimetablePdf(download, [1]);
		});
		const loading = getDocument({ data: new Uint8Array(bytes), useSystemFonts: false });
		const content = await (await (await loading.promise).getPage(1)).getTextContent();
		const text = content.items
			.map((item) => ('str' in item ? item.str : ''))
			.join('')
			.replace(/\s/g, '');
		assert.ok(text.includes('โรงเรียนทดสอบ'));
		assert.ok(text.includes(pageFixture().headerOwnerLabel.replace(/\s/g, '')));
		await loading.destroy();
	});
}

test('31 ordinary dense teacher timetables produce 31 complete full pages with one external header each', async () => {
	const pages = Array.from({ length: 31 }, (_, index) => {
		const page = pageFixture();
		page.headerOwnerLabel = `นายผู้สอนหมายเลข${index + 1} สกุลทดสอบ`;
		page.homeroomNames = { room: 'ม.1/1' };
		page.timetableBlocks = Array.from({ length: 50 }, (_, i) => ({
			blockKind: 'course',
			isActive: true,
			bellSchedulePeriodId: `period-${i % 10}`,
			dayOfWeek: ['MON', 'TUE', 'WED', 'THU', 'FRI'][Math.floor(i / 10)],
			offeringCode: 'ค21101',
			offeringName: 'คณิตศาสตร์',
			groups: [
				{
					homeroomIds: ['room'],
					homeroomNames: ['ม.1/1'],
					roomName: 'ห้องคณิตศาสตร์',
					instructors: []
				}
			],
			homerooms: [],
			teachers: []
		}));
		return page;
	});
	const bytes = await renderTimetable(pages, 'full', tallCrest);
	await withPdfArtifact(bytes, '31-teachers-full', async (download) => {
		await expectExternalTimetablePdfHeaders(download, 48 / 160, Array(31).fill(1));
		await expectSquareTimetablePdf(download, Array(31).fill(1));
	});
	const loading = getDocument({ data: new Uint8Array(bytes), useSystemFonts: false });
	const pdf = await loading.promise;
	assert.equal(pdf.numPages, 31);
	for (let i = 1; i <= 31; i++) {
		const content = await (await pdf.getPage(i)).getTextContent();
		const text = content.items
			.map((item) => ('str' in item ? item.str : ''))
			.join('')
			.replace(/\s/g, '');
		assert.ok(text.includes(pages[i - 1].headerOwnerLabel.replace(/\s/g, '')));
		assert.ok(text.includes('ศุกร์'));
		assert.ok(text.includes('SchoolOrbitTimeTable'));
	}
	await loading.destroy();
});

test('seven compact timetables paginate six then one without detached external headers', async () => {
	const bytes = await renderTimetable(
		Array.from({ length: 7 }, pageFixture),
		'portrait-2col',
		tallCrest
	);
	await withPdfArtifact(bytes, '7-teachers-compact', async (download) => {
		await expectExternalTimetablePdfHeaders(download, 48 / 160, [6, 1]);
		await expectSquareTimetablePdf(download, [6, 1]);
	});
});

test('a compact external header moves with its grid when the previous content leaves too little room', async () => {
	const bytes = await pdfmake
		.createPdf({
			pageSize: 'A4',
			pageMargins: [10, 30, 10, 20],
			defaultStyle: { font: 'THSarabunNew' },
			pageBreakBefore: builders.keepTimetableHeaderWithGrid,
			content: [
				{ text: 'PRECEDING_CONTENT', margin: [0, 0, 0, 720] },
				{
					columns: [
						{
							width: 282,
							stack: [
								builders.buildMiniTable(
									pageFixture(),
									282,
									{ dataUrl: tallCrest, aspectRatio: 48 / 160 },
									'โรงเรียนทดสอบ',
									0
								)
							]
						}
					]
				}
			]
		})
		.getBuffer();
	await withPdfArtifact(bytes, 'orphan-prevention', (download) =>
		expectExternalTimetablePdfHeaders(download, 48 / 160, [0, 1])
	);
});
