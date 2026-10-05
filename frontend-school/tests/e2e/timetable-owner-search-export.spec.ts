import { expect, test } from '@playwright/test';
import ExcelJS from 'exceljs';
import {
	installTimetableMock,
	makeTimetableBlock,
	makeSynchronizedTimetableBlock,
	makeStructuralTimetableBlock,
	timetableIds
} from './timetable-test-harness';

test.use({ serviceWorkers: 'block' });

function url(status: 'draft' | 'published' = 'draft') {
	return `/staff/academic/timetable?academicYearId=${timetableIds.year}&academicTermId=${timetableIds.term}&timetableVersionId=${status === 'draft' ? timetableIds.draftVersion : timetableIds.publishedVersion}&view=homeroom&ownerId=${timetableIds.homeroom}`;
}

for (const width of [1543, 390]) {
	for (const theme of ['light', 'dark']) {
		test(`owner search shows names and supports keyboard selection at ${width} ${theme}`, async ({
			page
		}) => {
			await page.setViewportSize({ width, height: 884 });
			await page.addInitScript((value) => localStorage.setItem('theme', value), theme);
			await installTimetableMock(page, {
				staff: [
					{
						id: timetableIds.teacherA,
						displayName: 'นางนัฎฐา สอดโคกสูง',
						status: 'active',
						subjectGroups: []
					},
					{
						id: timetableIds.teacherB,
						displayName: 'นางสาวอฤทัย ใจนวน',
						status: 'active',
						subjectGroups: []
					}
				]
			});
			await page.goto(url());
			const header = page.locator('header[aria-label="บริบทตารางสอน"]');
			const owner = header.getByRole('combobox', { name: 'เลือกรายการสำหรับจัดตาราง' });
			await expect(owner).toHaveText('ม.1/1');
			for (const [view, query, label] of [
				['ชั้น', 'ม.1/1', 'ม.1/1'],
				['กลุ่มเรียน', 'คณิตศาสตร์', 'ม.1/1 คณิตศาสตร์'],
				['ครู', 'ใจนวน', 'นางสาวอฤทัย ใจนวน']
			]) {
				await header.getByRole('button', { name: view, exact: true }).click();
				await owner.click();
				const search = page.getByPlaceholder('ค้นหาชั้น กลุ่มเรียน หรือชื่อครู...');
				await search.fill('ไม่มีรายการนี้');
				await expect(page.getByText('ไม่พบรายการที่ค้นหา')).toBeVisible();
				await search.fill(query);
				await expect(page.getByRole('option', { name: label, exact: true })).toBeVisible();
				await search.press('ArrowDown');
				await search.press('Enter');
				await expect(owner).toHaveText(label);
				await expect(owner).toHaveAttribute('aria-expanded', 'false');
			}
			await expect(page).toHaveURL(new RegExp(`ownerId=${timetableIds.teacherB}`));
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await page.screenshot({ path: `/tmp/timetable-load-search-${width}-${theme}.png` });
		});
	}
}

for (const status of ['draft', 'published'] as const) {
	test(`downloads correct subject groups and teacher totals from the selected ${status} version`, async ({
		page
	}) => {
		const math = {
			id: 'e1000000-0000-4000-8000-000000000201',
			name: 'คณิตศาสตร์',
			displayOrder: 1
		};
		const science = {
			id: 'e1000000-0000-4000-8000-000000000202',
			name: 'วิทยาศาสตร์',
			displayOrder: 2
		};
		const course = makeTimetableBlock(timetableIds.blockA, timetableIds.period1, {
			instructorIds: [timetableIds.teacherA, timetableIds.teacherB]
		});
		const sync = makeSynchronizedTimetableBlock(
			timetableIds.blockB,
			timetableIds.period2,
			[timetableIds.teacherA, timetableIds.teacherB],
			[timetableIds.teacherA, timetableIds.teacherB]
		);
		const special = makeStructuralTimetableBlock(
			timetableIds.createdBlock,
			timetableIds.period3,
			'ประชุมครู',
			[timetableIds.teacherA]
		);
		await installTimetableMock(page, {
			status,
			requestedWorkspaceVersion: true,
			blocks: [course, sync, special],
			staff: [
				{
					id: timetableIds.teacherA,
					displayName: 'ครูคณิตศาสตร์ A',
					status: 'active',
					subjectGroups: [math]
				},
				{
					id: timetableIds.teacherB,
					displayName: 'ครูคณิตศาสตร์ B',
					status: 'active',
					subjectGroups: [science]
				}
			],
			offeringSubjectGroups: [{ learningOfferingId: timetableIds.offeringA, subjectGroup: math }]
		});
		await page.goto(url(status));
		await page.getByRole('button', { name: 'ครู', exact: true }).click();
		const downloadPromise = page.waitForEvent('download');
		await page.getByRole('button', { name: 'สรุปคาบ XLSX', exact: true }).click();
		const download = await downloadPromise;
		expect(download.suggestedFilename()).toMatch(/\.xlsx$/);
		const path = await download.path();
		expect(path).toBeTruthy();
		const workbook = new ExcelJS.Workbook();
		await workbook.xlsx.readFile(path!);
		const summary = workbook.getWorksheet('สรุปต่อครู')!;
		const details = workbook.getWorksheet('รายละเอียด')!;
		const metadata = workbook.getWorksheet('ข้อมูลรายงาน')!;
		const rows = summary.getSheetValues().filter(Array.isArray) as ExcelJS.CellValue[][];
		const a = rows.find((row) => row[2] === 'ครูคณิตศาสตร์ A')!;
		const b = rows.find((row) => row[2] === 'ครูคณิตศาสตร์ B')!;
		expect(a[1]).toBe('คณิตศาสตร์');
		expect(a[3]).toBe(1);
		expect(a[8]).toBe(1);
		expect(a[11]).toBe(1);
		expect(a[12]).toBe(3);
		expect(b[1]).toBe('วิทยาศาสตร์');
		expect(b[6]).toBe(1);
		expect(b[8]).toBe(1);
		expect(b[12]).toBe(2);
		expect(details.rowCount).toBe(8); // Header, two subject group headings and five actual responsibilities.
		const detailRows = details.getSheetValues().filter(Array.isArray) as ExcelJS.CellValue[][];
		const courseA = detailRows.find(
			(row) => row[2] === 'ครูคณิตศาสตร์ A' && row[4] === 'วิชาในกลุ่มสาระ (ครูหลัก)'
		)!;
		expect(courseA[3]).toBe('คณิตศาสตร์');
		expect(courseA[9]).toBe('ม.1/1');
		expect(courseA[10]).toBe('ห้องคณิตศาสตร์');
		expect(detailRows.some((row) => row[11] === 'ประชุมครู')).toBe(true);
		expect(metadata.getCell('B3').value).toBe(
			status === 'draft' ? timetableIds.draftVersion : timetableIds.publishedVersion
		);
		expect(metadata.getCell('B4').value).toBe(status === 'draft' ? 'แบบร่าง' : 'เผยแพร่แล้ว');
		expect(summary.views[0]).toMatchObject({ state: 'frozen', ySplit: 1 });
	});
}
