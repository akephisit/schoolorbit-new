import { expect, test, type Download, type Page } from '@playwright/test';
import { PDFDocument } from 'pdf-lib';
import { getDocument } from 'pdfjs-dist/legacy/build/pdf.mjs';
import {
	expectSquareTimetablePdf,
	expectTimetablePdfPresentation,
	expectTimetablePdfTextColors,
	expectExternalTimetablePdfHeaders
} from './timetable-pdf-assertions';
import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import {
	installTimetableMock,
	makeTimetableBlock,
	makeStructuralTimetableBlock,
	makeSynchronizedTimetableBlock,
	timetableIds
} from './timetable-test-harness';

test.use({ serviceWorkers: 'block' });
const url = (view = 'homeroom', status = 'draft') =>
	`/staff/academic/timetable?academicYearId=${timetableIds.year}&academicTermId=${timetableIds.term}&timetableVersionId=${status === 'draft' ? timetableIds.draftVersion : timetableIds.publishedVersion}&view=${view}&ownerId=${view === 'teacher' ? timetableIds.teacherA : view === 'group' ? timetableIds.groupA : timetableIds.homeroom}`;

async function pdfText(download: Download) {
	const path = await download.path();
	expect(path).toBeTruthy();
	expect(readFileSync(path!).subarray(0, 5).toString()).toBe('%PDF-');
	await expectTimetablePdfPresentation(download, ['จันทร์', 'อังคาร', 'พุธ', 'พฤหัสฯ', 'ศุกร์']);
	return execFileSync('pdftotext', ['-raw', path!, '-'], { encoding: 'utf8' }).replace(/\s/g, '');
}

async function openDownload(page: Page) {
	await page.getByRole('button', { name: 'ดาวน์โหลด PDF', exact: true }).click();
	return page.getByRole('dialog', { name: 'ดาวน์โหลดตารางสอน PDF' });
}

async function confirmDownload(page: Page) {
	const dialog = page.getByRole('dialog', { name: 'ดาวน์โหลดตารางสอน PDF' });
	await dialog.getByRole('button', { name: /^ดาวน์โหลด \(/ }).click();
}

const teacherName = (id: string) =>
	id === timetableIds.teacherA ? 'นายพิสิษฐ สกุลทดสอบ' : 'นางสาวสายใจ สกุลทดสอบ';

for (const width of [1543, 390]) {
	for (const theme of ['light', 'dark']) {
		test(`downloads selected timetable PDF with complete cells at ${width}px ${theme}`, async ({
			page
		}) => {
			await page.setViewportSize({ width, height: 884 });
			const special = makeStructuralTimetableBlock(
				timetableIds.createdBlock,
				timetableIds.period3,
				'ประชุมครู',
				[timetableIds.teacherA]
			);
			special.homerooms[0].roomId = timetableIds.room;
			special.homerooms[0].roomCode = 'MATH-1';
			special.homerooms[0].roomName = 'ห้องคณิตศาสตร์';
			await installTimetableMock(page, {
				permissions: ['academic_timetable.read.school'],
				requestedWorkspaceVersion: true,
				staff: [timetableIds.teacherA, timetableIds.teacherB].map((id) => ({
					id,
					displayName: teacherName(id),
					subjectGroups: [],
					status: 'active'
				})),
				blocks: [
					makeTimetableBlock(timetableIds.blockA, timetableIds.period1, {
						instructorIds: [timetableIds.teacherA, timetableIds.teacherB]
					}),
					makeTimetableBlock(timetableIds.blockB, timetableIds.period1, {
						groupId: timetableIds.groupB,
						offeringId: timetableIds.offeringB,
						code: 'ว21101',
						name: 'วิทยาศาสตร์พื้นฐาน',
						instructorIds: [timetableIds.teacherB],
						roomId: timetableIds.roomB
					}),
					makeSynchronizedTimetableBlock(
						'c1000000-0000-4000-8000-000000000204',
						timetableIds.period2,
						[timetableIds.teacherA],
						[timetableIds.teacherB]
					),
					special
				].map((block) => ({
					...block,
					groups: block.groups.map((group) => ({
						...group,
						instructors: group.instructors.map((teacher) => ({
							...teacher,
							displayName: teacherName(teacher.teacherId)
						}))
					})),
					teachers: block.teachers.map((teacher) => ({
						...teacher,
						displayName: teacherName(teacher.teacherId)
					}))
				}))
			});
			await page.route('**/api/school/public', (route) =>
				route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify({
						success: true,
						data: { schoolName: 'โรงเรียนทดสอบ', logoFileId: null }
					})
				})
			);
			let brandingRequests = 0;
			page.on('request', (request) => {
				if (new URL(request.url()).pathname === '/api/school/public') brandingRequests++;
			});
			await page.goto(url());
			if (theme === 'dark') await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
			const downloadButton = page.getByRole('button', { name: 'ดาวน์โหลด PDF', exact: true });
			await expect(downloadButton).toBeEnabled();
			expect(brandingRequests).toBe(0);
			const courseCard = page.locator(
				`[data-timetable-lesson-card][data-block-id="${timetableIds.blockA}"]`
			);
			await expect(courseCard).toContainText('ครูพิสิษฐ +1');
			await expect(courseCard).not.toContainText('สกุลทดสอบ');
			await expect(page.getByRole('button', { name: 'แก้ไข', exact: true })).toHaveCount(0);
			await page.screenshot({
				path: `/tmp/academic-timetable-pdf-${width}-${theme}.png`,
				fullPage: true,
				animations: 'disabled'
			});
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			for (const view of ['ชั้น', 'กลุ่มเรียน', 'ครู', 'โรงเรียน']) {
				await page
					.locator('header[aria-label="บริบทตารางสอน"]')
					.getByRole('button', { name: view, exact: true })
					.click();
				const brandingBefore = brandingRequests;
				const pending = page.waitForEvent('download');
				await openDownload(page);
				await confirmDownload(page);
				const download = await pending;
				await expectSquareTimetablePdf(download, [1]);
				expect(download.suggestedFilename()).toMatch(/แบบร่าง\.pdf$/);
				await download.saveAs(`/tmp/academic-timetable-${width}-${theme}-${view}.pdf`);
				const text = await pdfText(download);
				expect(brandingRequests).toBe(brandingBefore + 1);
				expect(text).toContain(
					`โรงเรียนทดสอบ${view === 'ครู' ? 'ตารางสอน' : 'ตารางเรียน'}ภาคเรียนที่1ปีการศึกษา2569`
				);
				if (view === 'ชั้น' || view === 'โรงเรียน')
					expect(text).toContain('ปีการศึกษา2569ระดับชั้นมัธยมศึกษาปีที่1/1');
				if (view === 'กลุ่มเรียน') expect(text).toContain('ปีการศึกษา2569ม.1/1คณิตศาสตร์');
				expect(text).toContain('ค21101');
				await expectTimetablePdfTextColors(download, {
					ค21101: '#1e3a8a',
					...(view === 'กลุ่มเรียน' ? { ชุมนุม: '#047857' } : { ประชุมครู: '#047857' }),
					ห้องคณิตศาสตร์: '#1f2937',
					...(view === 'ครู' ? { 'ม.1/1': '#d97706' } : { ครูพิสิษฐ: '#4b5563' })
				});
				expect(text).toContain('ห้องคณิตศาสตร์');
				expect(text).not.toMatch(/MATH-1|LAB-2/);
				if (view === 'ชั้น' || view === 'โรงเรียน') {
					expect(text).toContain('ว21101');
					expect(text).toContain('ครูพิสิษฐสกุลทดสอบ,ครูสายใจสกุลทดสอบ');
					// Teacher A appears on the course, but not again on the special period.
					expect(text.match(/ครูพิสิษฐ/g)).toHaveLength(1);
					expect(text).toContain('ประชุมครู');
				}
				if (view === 'ครู') {
					expect(download.suggestedFilename()).toContain('ครูพิสิษฐ สกุลทดสอบ');
					expect(text).toContain(
						'โรงเรียนทดสอบตารางสอนภาคเรียนที่1ปีการศึกษา2569นายพิสิษฐสกุลทดสอบ'
					);
					expect(text).toContain('ประชุมครู');
					expect(text.match(/ม\.1\/1/g)).toHaveLength(1);
					expect(text).not.toContain('ม.1/1คณิตศาสตร์');
				}
				expect(text).not.toContain('แบบร่าง');
				expect(text).not.toContain('เผยแพร่แล้ว');
				expect(text).not.toContain('CLUB-');
				expect(text).toContain('สกุลทดสอบ');
				await expect(downloadButton).toBeEnabled();
				if (view === 'ชั้น' || view === 'ครู') {
					const dialog = await openDownload(page);
					await dialog.getByRole('button', { name: '6 ตารางต่อหน้า', exact: true }).click();
					const compactPending = page.waitForEvent('download');
					await confirmDownload(page);
					const compactDownload = await compactPending;
					await expectSquareTimetablePdf(compactDownload, [1]);
					expect(compactDownload.suggestedFilename()).not.toBe(download.suggestedFilename());
					expect(compactDownload.suggestedFilename()).toContain('6ต่อหน้า');
					await compactDownload.saveAs(`/tmp/pdf-options-${width}-${theme}-${view}.pdf`);
					const compactText = await pdfText(compactDownload);
					await expectTimetablePdfTextColors(compactDownload, {
						ค21101: '#1e3a8a',
						ประชุมครู: '#047857',
						ห้องคณิตศาสตร์: '#1f2937',
						...(view === 'ครู' ? { 'ม.1/1': '#d97706' } : { ครูพิสิษฐ: '#4b5563' })
					});
					expect(brandingRequests).toBe(brandingBefore + 2);
					if (view === 'ชั้น') {
						expect(compactText).toContain('ครูพิสิษฐสกุลทดสอบ,ครูสายใจสกุลทดสอบ');
					} else {
						expect(compactText.match(/ม\.1\/1/g)).toHaveLength(1);
						expect(compactText).not.toContain('ม.1/1คณิตศาสตร์');
					}
					expect(compactText).not.toContain('แบบร่าง');
					expect(compactText).toContain('สกุลทดสอบ');
					expect(compactText.match(/ครูพิสิษฐ/g) ?? []).toHaveLength(view === 'ครู' ? 0 : 1);
					expect(compactText).toContain(
						`โรงเรียนทดสอบ${view === 'ครู' ? 'ตารางสอน' : 'ตารางเรียน'}ภาคเรียนที่1ปีการศึกษา2569${view === 'ครู' ? 'นายพิสิษฐสกุลทดสอบ' : 'ระดับชั้นมัธยมศึกษาปีที่1/1'}`
					);
					expect(compactText).toContain('ประชุมครู');
				}
			}
		});
	}
}

test('PDF failure preserves the table and permits retry', async ({ page }) => {
	await installTimetableMock(page, {
		status: 'published',
		requestedWorkspaceVersion: true,
		blocks: [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)]
	});
	let attempts = 0;
	await page.route('**/api/school/public', (route) =>
		route.fulfill({
			status: ++attempts === 1 ? 500 : 200,
			contentType: 'application/json',
			body: JSON.stringify(
				attempts === 1
					? { success: false, error: 'โหลดตราโรงเรียนไม่สำเร็จ' }
					: { success: true, data: { schoolName: 'โรงเรียนทดสอบ', logoFileId: null } }
			)
		})
	);
	await page.goto(url('homeroom', 'published'));
	const button = page.getByRole('button', { name: 'ดาวน์โหลด PDF', exact: true });
	await openDownload(page);
	await confirmDownload(page);
	await expect(page.getByText('โหลดตราโรงเรียนไม่สำเร็จ')).toBeVisible();
	await expect(button).toBeEnabled();
	await expect(page.getByText('ค21101').first()).toBeVisible();
	const pending = page.waitForEvent('download');
	await confirmDownload(page);
	const download = await pending;
	expect(download.suggestedFilename()).toMatch(/เผยแพร่แล้ว\.pdf$/);
	const text = await pdfText(download);
	expect(text).toContain('ภาคเรียนที่1');
	expect(text).not.toContain('เผยแพร่แล้ว');
	expect(text).not.toContain('2026-05-01');
});

test('PDF captures the requested owner and version before its lazy renderer loads', async ({
	page
}) => {
	await installTimetableMock(page, {
		requestedWorkspaceVersion: true,
		blocks: [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)]
	});
	await page.route('**/api/school/public', (route) =>
		route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify({
				success: true,
				data: { schoolName: 'โรงเรียนทดสอบ', logoFileId: null }
			})
		})
	);
	await page.goto(url());
	const button = page.getByRole('button', { name: 'ดาวน์โหลด PDF', exact: true });
	await expect(button).toBeEnabled();
	await openDownload(page);
	let release!: () => void;
	let start!: () => void;
	const gate = new Promise<void>((resolve) => (release = resolve));
	const started = new Promise<void>((resolve) => (start = resolve));
	await page.route('**/_app/immutable/chunks/*.js', async (route) => {
		start();
		await gate;
		await route.continue();
	});
	const pending = page.waitForEvent('download');
	await confirmDownload(page);
	await started;
	await expect(button).toBeDisabled();
	const dialog = page.getByRole('dialog', { name: 'ดาวน์โหลดตารางสอน PDF' });
	await expect(dialog.getByRole('button', { name: 'กำลังดาวน์โหลด...' })).toBeDisabled();
	await page.keyboard.press('Escape');
	await expect(dialog).toBeVisible();

	release();
	const download = await pending;
	expect(download.suggestedFilename()).toMatch(/^ตารางเรียน ม\.1-1 .*แบบร่าง\.pdf$/);
	const text = await pdfText(download);
	expect(text).toContain('ระดับชั้นมัธยมศึกษาปีที่1/1');
	expect(text).not.toContain('แบบร่าง');
	await expect(button).toBeEnabled();
});

test('PDF stays disabled until the workspace arrives and when the timetable is empty', async ({
	page
}) => {
	await installTimetableMock(page, { blocks: [] });
	let release!: () => void;
	let start!: () => void;
	const gate = new Promise<void>((resolve) => (release = resolve));
	const started = new Promise<void>((resolve) => (start = resolve));
	await page.route('**/api/academic/timetable-blocks/workspace?*', async (route) => {
		start();
		await gate;
		await route.fallback();
	});
	await page.goto(url());
	await started;
	const button = page.getByRole('button', { name: 'ดาวน์โหลด PDF', exact: true });
	await expect(button).toBeDisabled();
	release();
	await expect(page.getByRole('combobox', { name: 'เลือกรายการสำหรับจัดตาราง' })).toBeVisible();
	await expect(button).toBeDisabled();
});

for (const width of [1543, 390]) {
	for (const theme of ['light', 'dark']) {
		test(`selects PDF owners and paginates compact teacher tables at ${width}px ${theme}`, async ({
			page
		}) => {
			await page.setViewportSize({ width, height: 884 });
			const staff = [
				timetableIds.teacherA,
				timetableIds.teacherB,
				...Array.from(
					{ length: 5 },
					(_, i) => `c1000000-0000-4000-8000-${String(301 + i).padStart(12, '0')}`
				)
			].map((id, i) => ({
				id,
				displayName: i === 0 ? 'นายพิสิษฐ สกุลทดสอบ' : `นางสาวผู้สอน${i} สกุล${i}`,
				subjectGroups: [],
				status: 'active'
			}));
			await installTimetableMock(page, {
				permissions: ['academic_timetable.read.school'],
				homerooms: [1, 2].map((number) => ({
					id: number === 1 ? timetableIds.homeroom : '81000000-0000-4000-8000-000000000202',
					code: `M1-${number}`,
					name: `ม.1/${number}`,
					gradeLevelId: '81000000-0000-4000-8000-000000000301',
					gradeLevelType: 'secondary',
					gradeLevelYear: 1,
					roomNumber: String(number),
					isActive: true
				})),
				staff,
				blocks: [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)]
			});
			await page.route('**/api/school/public', (route) =>
				route.fulfill({
					status: 200,
					contentType: 'application/json',
					body: JSON.stringify({
						success: true,
						data: { schoolName: 'โรงเรียนทดสอบ', logoFileId: null }
					})
				})
			);
			await page.goto(url());
			if (theme === 'dark') await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
			let dialog = await openDownload(page);
			await expect(dialog.getByRole('checkbox', { name: 'ม.1/1' })).toBeChecked();
			await dialog.getByRole('button', { name: 'เลือกทั้งหมด', exact: true }).click();
			await expect(dialog.getByRole('checkbox')).toHaveCount(2);
			await dialog.getByRole('checkbox', { name: 'ม.1/2' }).uncheck();
			await expect(dialog.getByRole('status')).toHaveText('เลือกแล้ว 1 / 2 รายการ');
			await dialog.getByRole('button', { name: 'ล้างการเลือก', exact: true }).click();
			await expect(dialog.getByRole('button', { name: 'ดาวน์โหลด (0)' })).toBeDisabled();
			await dialog.getByRole('button', { name: 'เลือกทั้งหมด', exact: true }).click();
			let pending = page.waitForEvent('download');
			await confirmDownload(page);
			let download = await pending;
			let pdf = await PDFDocument.load(readFileSync((await download.path())!));
			expect(pdf.getPageCount()).toBe(2);
			await expectSquareTimetablePdf(download, [1, 1]);
			const rooms = await pdfText(download);
			expect(rooms).toContain('ระดับชั้นมัธยมศึกษาปีที่1/1');
			expect(rooms).toContain('ระดับชั้นมัธยมศึกษาปีที่1/2');
			dialog = await openDownload(page);
			await dialog.getByRole('button', { name: 'ครู', exact: true }).click();
			await expect(dialog.getByRole('checkbox')).toHaveCount(7);
			await dialog.getByRole('button', { name: 'เลือกทั้งหมด', exact: true }).click();
			await dialog.getByRole('textbox', { name: 'ค้นหารายการดาวน์โหลด' }).fill('สกุล6');
			await expect(dialog.getByRole('checkbox')).toHaveCount(1);
			await dialog.getByRole('checkbox').uncheck();
			await dialog.getByRole('textbox', { name: 'ค้นหารายการดาวน์โหลด' }).fill('ไม่ตรงกับรายชื่อ');
			await expect(dialog.getByText('ไม่พบรายการที่ค้นหา')).toBeVisible();
			await dialog.getByRole('textbox', { name: 'ค้นหารายการดาวน์โหลด' }).fill('');
			await expect(dialog.getByRole('checkbox')).toHaveCount(7);
			await expect(dialog.getByRole('status')).toHaveText('เลือกแล้ว 6 / 7 รายการ');
			await dialog.getByRole('button', { name: '6 ตารางต่อหน้า', exact: true }).click();
			await page.screenshot({
				path: `/tmp/pdf-options-${width}-${theme}.png`,
				fullPage: true,
				animations: 'disabled'
			});
			const bounds = await dialog.boundingBox();
			expect(bounds!.x).toBeGreaterThanOrEqual(0);
			expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(884);
			await expect(dialog.getByRole('button', { name: 'ดาวน์โหลด (6)' })).toBeInViewport();
			pending = page.waitForEvent('download');
			await confirmDownload(page);
			download = await pending;
			await download.saveAs(`/tmp/pdf-options-${width}-${theme}-6.pdf`);
			pdf = await PDFDocument.load(readFileSync((await download.path())!));
			expect(pdf.getPageCount()).toBe(1);
			await expectSquareTimetablePdf(download, [6]);
			let text = await pdfText(download);
			expect(text).toContain('นายพิสิษฐ');
			for (const teacher of staff.slice(0, 6)) {
				expect(text).toContain(
					`โรงเรียนทดสอบตารางสอนภาคเรียนที่1ปีการศึกษา2569${teacher.displayName.replace(/\s/g, '')}`
				);
			}
			expect(text).toContain('สกุลทดสอบ');
			expect(text).not.toContain('นางสาวผู้สอน6');
			dialog = await openDownload(page);
			await dialog.getByRole('button', { name: 'ครู', exact: true }).click();
			await dialog.getByRole('button', { name: 'เลือกทั้งหมด', exact: true }).click();
			await dialog.getByRole('button', { name: '6 ตารางต่อหน้า', exact: true }).click();
			pending = page.waitForEvent('download');
			await confirmDownload(page);
			download = await pending;
			pdf = await PDFDocument.load(readFileSync((await download.path())!));
			expect(pdf.getPageCount()).toBe(2);
			await expectSquareTimetablePdf(download, [6, 1]);
			text = await pdfText(download);
			expect(text).toContain('นางสาวผู้สอน6');
			for (const teacher of staff) {
				expect(text).toContain(
					`โรงเรียนทดสอบตารางสอนภาคเรียนที่1ปีการศึกษา2569${teacher.displayName.replace(/\s/g, '')}`
				);
			}
			await openDownload(page);
			await page.keyboard.press('Escape');
			await expect(page.getByRole('dialog')).toHaveCount(0);
			await expect(page.getByRole('button', { name: 'ดาวน์โหลด PDF', exact: true })).toBeFocused();
		});
	}
}

for (const [shape, width, height] of [
	['tall', 48, 160],
	['wide', 160, 48]
] as const) {
	test(`places a ${shape} crest beside external headers in dense full and six-up PDFs`, async ({
		page
	}) => {
		const fixture = await PDFDocument.create();
		// Generate an opaque, distinctive crest in the browser without another image dependency.
		await installTimetableMock(page, {
			requestedWorkspaceVersion: true,
			permissions: ['academic_timetable.read.school'],
			periodCount: 10,
			staff: Array.from({ length: 6 }, (_, i) => ({
				id:
					i === 0
						? timetableIds.teacherA
						: `c1000000-0000-4000-8000-${String(401 + i).padStart(12, '0')}`,
				displayName: i === 0 ? 'นายพิสิษฐ สกุลทดสอบ' : `นางสาวผู้สอน${i} สกุล${i}`,
				subjectGroups: [],
				status: 'active'
			})),
			blocks: Array.from({ length: 50 }, (_, i) => ({
				...makeTimetableBlock(
					`c1000000-0000-4000-8000-${String(501 + i).padStart(12, '0')}`,
					`41000000-0000-4000-8000-${String(201 + (i % 10)).padStart(12, '0')}`
				),
				dayOfWeek: ['MON', 'TUE', 'WED', 'THU', 'FRI'][Math.floor(i / 10)]
			}))
		});
		const logoId = '80000000-0000-4000-8000-000000000001';
		let brandingReads = 0;
		await page.route('**/api/school/public', (route) => {
			brandingReads++;
			return route.fulfill({
				json: { success: true, data: { schoolName: 'โรงเรียนทดสอบ', logoFileId: logoId } }
			});
		});
		await page.route(`**/api/public/files/${logoId}/delivery`, (route) =>
			route.fulfill({
				json: { success: true, data: { url: 'https://public-files.example.test/crest.png' } }
			})
		);
		await page.route('https://public-files.example.test/crest.png', (route) =>
			route.fulfill({
				contentType: 'image/png',
				headers: { 'access-control-allow-origin': '*' },
				body: logoBytes
			})
		);
		await page.goto(url('teacher'));
		const data = await page.evaluate(
			({ width, height }) => {
				const canvas = document.createElement('canvas');
				canvas.width = width;
				canvas.height = height;
				const context = canvas.getContext('2d')!;
				context.fillStyle = '#c21d3d';
				context.fillRect(0, 0, width, height);
				context.fillStyle = '#1455c0';
				context.fillRect(width / 4, height / 4, width / 2, height / 2);
				return canvas.toDataURL('image/png').split(',')[1];
			},
			{ width, height }
		);
		const logoBytes = Buffer.from(data, 'base64');
		// Confirm the synthetic asset really carries the intended aspect ratio.
		const image = await fixture.embedPng(logoBytes);
		expect(image.width / image.height).toBe(width / height);
		await openDownload(page);
		let pending = page.waitForEvent('download');
		await confirmDownload(page);
		let download = await pending;
		await download.saveAs(`/tmp/timetable-dense-${shape}-full.pdf`);
		await expectSquareTimetablePdf(download, [1]);
		await expectTimetablePdfPresentation(download, ['จันทร์', 'อังคาร', 'พุธ', 'พฤหัสฯ', 'ศุกร์']);
		await expectExternalTimetablePdfHeaders(download, width / height, [1]);
		await expectTimetablePdfTextColors(download, {
			ค21101: '#1e3a8a',
			'ม.1/1': '#d97706',
			ห้องคณิตศาสตร์: '#1f2937'
		});
		const dialog = await openDownload(page);
		await dialog.getByRole('button', { name: 'ครู', exact: true }).click();
		await dialog.getByRole('button', { name: 'เลือกทั้งหมด', exact: true }).click();
		await dialog.getByRole('button', { name: '6 ตารางต่อหน้า', exact: true }).click();
		pending = page.waitForEvent('download');
		await confirmDownload(page);
		download = await pending;
		await download.saveAs(`/tmp/timetable-dense-${shape}-six.pdf`);
		await expectSquareTimetablePdf(download, [6]);
		await expectTimetablePdfPresentation(download, ['จันทร์', 'อังคาร', 'พุธ', 'พฤหัสฯ', 'ศุกร์']);
		await expectExternalTimetablePdfHeaders(download, width / height, [6]);
		await expectTimetablePdfTextColors(download, {
			ค21101: '#1e3a8a',
			'ม.1/1': '#d97706',
			ห้องคณิตศาสตร์: '#1f2937'
		});
		expect(brandingReads).toBe(2);
	});
}

test('exports 31 selected teachers as 31 complete pages with an external crest and heading on each', async ({
	page
}) => {
	test.setTimeout(60_000);
	const staff = Array.from({ length: 31 }, (_, i) => ({
		id:
			i === 0
				? timetableIds.teacherA
				: `c1000000-0000-4000-8000-${String(701 + i).padStart(12, '0')}`,
		displayName: `นายผู้สอนหมายเลข${i + 1} สกุลทดสอบ`,
		subjectGroups: [],
		status: 'active'
	}));
	await installTimetableMock(page, {
		requestedWorkspaceVersion: true,
		permissions: ['academic_timetable.read.school'],
		periodCount: 10,
		staff,
		blocks: staff.map((teacher, i) =>
			makeTimetableBlock(
				`c1000000-0000-4000-8000-${String(801 + i).padStart(12, '0')}`,
				timetableIds.period1,
				{ instructorIds: [teacher.id] }
			)
		)
	});
	const logoId = '80000000-0000-4000-8000-000000000001';
	const logoBytes = readFileSync(new URL('../../static/notification-badge.png', import.meta.url));
	const asset = await (await PDFDocument.create()).embedPng(logoBytes);
	let brandingReads = 0;
	await page.route('**/api/school/public', (route) => {
		brandingReads++;
		return route.fulfill({
			json: { success: true, data: { schoolName: 'โรงเรียนทดสอบ', logoFileId: logoId } }
		});
	});
	await page.route(`**/api/public/files/${logoId}/delivery`, (route) =>
		route.fulfill({
			json: { success: true, data: { url: 'https://public-files.example.test/batch-crest.png' } }
		})
	);
	await page.route('https://public-files.example.test/batch-crest.png', (route) =>
		route.fulfill({
			contentType: 'image/png',
			headers: { 'access-control-allow-origin': '*' },
			body: logoBytes
		})
	);
	await page.goto(url('teacher'));
	const dialog = await openDownload(page);
	await dialog.getByRole('button', { name: 'ครู', exact: true }).click();
	await dialog.getByRole('button', { name: 'เลือกทั้งหมด', exact: true }).click();
	await expect(dialog.getByRole('status')).toHaveText('เลือกแล้ว 31 / 31 รายการ');
	const pending = page.waitForEvent('download');
	await confirmDownload(page);
	const download = await pending;
	await download.saveAs('/tmp/timetable-external-31-teachers-browser.pdf');
	await expectSquareTimetablePdf(download, Array(31).fill(1));
	await expectExternalTimetablePdfHeaders(download, asset.width / asset.height, Array(31).fill(1));
	await expectTimetablePdfPresentation(download, ['จันทร์', 'อังคาร', 'พุธ', 'พฤหัสฯ', 'ศุกร์']);
	const pdf = await PDFDocument.load(readFileSync((await download.path())!));
	expect(pdf.getPageCount()).toBe(31);
	const text = await pdfText(download);
	for (const teacher of staff) expect(text).toContain(teacher.displayName.replace(/\s/g, ''));
	expect(brandingReads).toBe(1);
});

test('wraps school and owner using their loaded bold measurement face within the mini header', async ({
	page
}) => {
	const schoolName = `โรงเรียน${'พัฒนา'.repeat(13)}วิทยาคม`;
	const ownerName = `นาย${'พัฒนา'.repeat(13)}วิทยาคม`;
	await installTimetableMock(page, {
		requestedWorkspaceVersion: true,
		permissions: ['academic_timetable.read.school'],
		periodCount: 10,
		staff: [
			{ id: timetableIds.teacherA, displayName: ownerName, subjectGroups: [], status: 'active' }
		],
		blocks: [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)]
	});
	const logoId = '80000000-0000-4000-8000-000000000001';
	const logoBytes = readFileSync(new URL('../../static/notification-badge.png', import.meta.url));
	const crest = await (await PDFDocument.create()).embedPng(logoBytes);
	expect(crest.width / crest.height).toBe(1);
	await page.route('**/api/school/public', (route) =>
		route.fulfill({
			json: { success: true, data: { schoolName, logoFileId: logoId } }
		})
	);
	await page.route(`**/api/public/files/${logoId}/delivery`, (route) =>
		route.fulfill({
			json: { success: true, data: { url: 'https://public-files.example.test/bold-crest.png' } }
		})
	);
	await page.route('https://public-files.example.test/bold-crest.png', (route) =>
		route.fulfill({
			contentType: 'image/png',
			headers: { 'access-control-allow-origin': '*' },
			body: logoBytes
		})
	);
	await page.goto(url('teacher'));
	const dialog = await openDownload(page);
	await dialog.getByRole('button', { name: 'ครู', exact: true }).click();
	await dialog.getByRole('button', { name: 'เลือกทั้งหมด', exact: true }).click();
	await expect(dialog.getByRole('status')).toHaveText('เลือกแล้ว 1 / 1 รายการ');
	await dialog.getByRole('button', { name: '6 ตารางต่อหน้า', exact: true }).click();
	const pending = page.waitForEvent('download');
	await confirmDownload(page);
	const download = await pending;
	await download.saveAs('/tmp/timetable-external-bold-browser-boundary.pdf');
	const faces = await page.evaluate(() =>
		Array.from(document.fonts)
			.filter((face) => face.family === 'THSarabunNewMeasure')
			.map((face) => ({ weight: face.weight, status: face.status }))
	);
	expect(faces).toHaveLength(2);
	expect(faces.every((face) => face.status === 'loaded')).toBe(true);
	expect(faces.some((face) => face.weight === 'bold' || face.weight === '700')).toBe(true);
	expect(faces.some((face) => face.weight === 'normal' || face.weight === '400')).toBe(true);
	await expectSquareTimetablePdf(download, [1]);
	await expectExternalTimetablePdfHeaders(download, 1, [1]);
	const loading = getDocument({
		data: new Uint8Array(readFileSync((await download.path())!)),
		useSystemFonts: false
	});
	try {
		const pdf = await loading.promise;
		expect(pdf.numPages).toBe(1);
		const content = await (await pdf.getPage(1)).getTextContent();
		const header = content.items.filter(
			(item) =>
				'str' in item &&
				item.str.trim() &&
				Math.abs(item.height - 9) < 0.01 &&
				item.transform[4] < 300
		);
		expect(header.length).toBeGreaterThanOrEqual(5);
		for (const item of header) {
			if (!('str' in item)) continue;
			expect(item.transform[4], item.str).toBeGreaterThanOrEqual(10 - 0.01);
			expect(item.transform[4] + item.width, item.str).toBeLessThanOrEqual(291 + 0.01);
		}
		const text = header
			.map((item) => ('str' in item ? item.str : ''))
			.join('')
			.replace(/\s/g, '');
		expect(text).toContain(schoolName);
		expect(text).toContain(ownerName);
	} finally {
		await loading.destroy();
	}
});
