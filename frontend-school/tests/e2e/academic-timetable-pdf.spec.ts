import { expect, test, type Download, type Page } from '@playwright/test';
import { PDFDocument } from 'pdf-lib';
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
				const pending = page.waitForEvent('download');
				await openDownload(page);
				await confirmDownload(page);
				const download = await pending;
				expect(download.suggestedFilename()).toMatch(/แบบร่าง\.pdf$/);
				await download.saveAs(`/tmp/academic-timetable-${width}-${theme}-${view}.pdf`);
				const text = await pdfText(download);
				expect(text).toContain('ค21101');
				expect(text).toContain('ห้องคณิตศาสตร์');
				if (view === 'ชั้น' || view === 'โรงเรียน') {
					expect(text).toContain('ว21101');
					expect(text).toContain('ครูพิสิษฐ,ครูสายใจ');
					// Teacher A appears on the course, but not again on the special period.
					expect(text.match(/ครูพิสิษฐ/g)).toHaveLength(1);
					expect(text).toContain('ประชุมครู');
				}
				if (view === 'ครู') {
					expect(text).toContain('ประชุมครู');
					expect(text.match(/ม\.1\/1/g)).toHaveLength(1);
					expect(text).not.toContain('ม.1/1คณิตศาสตร์');
				}
				expect(text).not.toContain('แบบร่าง');
				expect(text).not.toContain('เผยแพร่แล้ว');
				expect(text).not.toContain('CLUB-');
				expect(text).not.toContain('สกุลทดสอบ');
				await expect(downloadButton).toBeEnabled();
				if (view === 'ชั้น' || view === 'ครู') {
					const dialog = await openDownload(page);
					await dialog.getByRole('button', { name: '6 ตารางต่อหน้า', exact: true }).click();
					const compactPending = page.waitForEvent('download');
					await confirmDownload(page);
					const compactDownload = await compactPending;
					await compactDownload.saveAs(`/tmp/pdf-options-${width}-${theme}-${view}.pdf`);
					const compactText = await pdfText(compactDownload);
					if (view === 'ชั้น') {
						expect(compactText).toContain('ครูพิสิษฐ,ครูสายใจ');
					} else {
						expect(compactText.match(/ม\.1\/1/g)).toHaveLength(1);
						expect(compactText).not.toContain('ม.1/1คณิตศาสตร์');
					}
					expect(compactText).not.toContain('แบบร่าง');
					expect(compactText).not.toContain('สกุลทดสอบ');
					expect(compactText.match(/ครูพิสิษฐ/g)).toHaveLength(1);
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
	expect(text).toContain('ม.1/1');
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
			const rooms = await pdfText(download);
			expect(rooms).toContain('ม.1/1');
			expect(rooms).toContain('ม.1/2');
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
			let text = await pdfText(download);
			expect(text).toContain('ครูพิสิษฐ');
			expect(text).not.toContain('สกุลทดสอบ');
			expect(text).not.toContain('ครูผู้สอน6');
			dialog = await openDownload(page);
			await dialog.getByRole('button', { name: 'ครู', exact: true }).click();
			await dialog.getByRole('button', { name: 'เลือกทั้งหมด', exact: true }).click();
			await dialog.getByRole('button', { name: '6 ตารางต่อหน้า', exact: true }).click();
			pending = page.waitForEvent('download');
			await confirmDownload(page);
			download = await pending;
			pdf = await PDFDocument.load(readFileSync((await download.path())!));
			expect(pdf.getPageCount()).toBe(2);
			text = await pdfText(download);
			expect(text).toContain('ครูผู้สอน6');
			await openDownload(page);
			await page.keyboard.press('Escape');
			await expect(page.getByRole('dialog')).toHaveCount(0);
			await expect(page.getByRole('button', { name: 'ดาวน์โหลด PDF', exact: true })).toBeFocused();
		});
	}
}
