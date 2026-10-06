import { expect, test, type Download } from '@playwright/test';
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
	return execFileSync('pdftotext', ['-layout', path!, '-'], { encoding: 'utf8' }).replace(
		/\s/g,
		''
	);
}

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
				]
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
			await expect(page.getByRole('button', { name: 'แก้ไข', exact: true })).toHaveCount(0);
			await page.screenshot({
				path: `/tmp/academic-timetable-pdf-${width}-${theme}.png`,
				fullPage: true
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
				await downloadButton.click();
				const download = await pending;
				expect(download.suggestedFilename()).toMatch(/แบบร่าง\.pdf$/);
				await download.saveAs(`/tmp/academic-timetable-${width}-${theme}-${view}.pdf`);
				const text = await pdfText(download);
				expect(text).toContain('ค21101');
				expect(text).toContain('ห้องคณิตศาสตร์');
				if (view === 'ชั้น' || view === 'โรงเรียน') {
					expect(text).toContain('ว21101');
					expect(text).toContain('ครูคณิตศาสตร์A,ครูคณิตศาสตร์B');
					expect(text).toContain('ประชุมครู');
				}
				if (view === 'ครู') expect(text).toContain('ประชุมครู');
				expect(text).not.toContain('CLUB-');
				await expect(downloadButton).toBeEnabled();
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
	await button.click();
	await expect(page.getByText('โหลดตราโรงเรียนไม่สำเร็จ')).toBeVisible();
	await expect(button).toBeEnabled();
	await expect(page.getByText('ค21101').first()).toBeVisible();
	const pending = page.waitForEvent('download');
	await button.click();
	const download = await pending;
	expect(download.suggestedFilename()).toMatch(/เผยแพร่แล้ว\.pdf$/);
	expect(await pdfText(download)).toContain('เริ่ม2026-05-01');
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
	await button.click();
	await started;
	await expect(button).toBeDisabled();
	await page
		.locator('header[aria-label="บริบทตารางสอน"]')
		.getByRole('button', { name: 'ครู', exact: true })
		.click();
	await page.getByRole('button', { name: 'เลือกรุ่นตารางสอน', exact: true }).click();
	await page.getByRole('option', { name: /เผยแพร่/ }).click();
	await expect(page).toHaveURL(new RegExp(timetableIds.publishedVersion));
	release();
	const download = await pending;
	expect(download.suggestedFilename()).toMatch(/^ตารางเรียน ม\.1-1 .*แบบร่าง\.pdf$/);
	expect(await pdfText(download)).toContain('แบบร่าง');
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
