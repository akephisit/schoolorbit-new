import { expect, test } from '@playwright/test';

import { installTimetableMock, makeTimetableBlock, timetableIds } from './timetable-test-harness';

test.use({ serviceWorkers: 'block', viewport: { width: 1920, height: 1080 } });

function boardUrl(): string {
	return (
		`/staff/academic/timetable?academicYearId=${timetableIds.year}` +
		`&academicTermId=${timetableIds.term}` +
		`&timetableVersionId=${timetableIds.draftVersion}` +
		`&view=homeroom&ownerId=${timetableIds.homeroom}`
	);
}

function wholeSchoolUrl(): string {
	return (
		`/staff/academic/timetable?academicYearId=${timetableIds.year}` +
		`&academicTermId=${timetableIds.term}` +
		`&timetableVersionId=${timetableIds.draftVersion}&view=wholeSchool`
	);
}

async function installDenseWorkspace(page: Parameters<typeof installTimetableMock>[0]) {
	const shortBlock = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const tallBlock = makeTimetableBlock(timetableIds.blockB, timetableIds.period2, {
		groupId: timetableIds.groupB,
		offeringId: timetableIds.offeringB,
		code: 'ว21101',
		name: 'วิทยาศาสตร์พื้นฐานและการออกแบบเทคโนโลยีแบบบูรณาการ'
	});
	tallBlock.groups[0].instructors[0].displayName =
		'คุณครูผู้สอนวิทยาศาสตร์และเทคโนโลยีชื่อยาวสำหรับทดสอบ';

	await installTimetableMock(page, {
		blocks: [shortBlock, tallBlock],
		periodCount: 10,
		requiredPeriods: 2
	});
}

async function installDenseBoard(page: Parameters<typeof installTimetableMock>[0]) {
	await installDenseWorkspace(page);
	await page.goto(boardUrl());
}

test('keeps ten equal period columns within the desktop timetable board', async ({ page }) => {
	await installDenseBoard(page);

	const board = page.getByRole('region', { name: 'ตารางของ ม.1/1' });
	await expect(board).toBeVisible();
	await expect(board.getByRole('columnheader')).toHaveCount(11);
	for (const [shortDay, fullDay] of [
		['จ.', 'วันจันทร์'],
		['อ.', 'วันอังคาร'],
		['พ.', 'วันพุธ'],
		['พฤ.', 'วันพฤหัสบดี'],
		['ศ.', 'วันศุกร์']
	]) {
		const rowHeader = board.locator('tbody th', { hasText: shortDay });
		await expect(rowHeader).toHaveText(shortDay);
		await expect(rowHeader).toHaveAttribute('aria-label', fullDay);
	}

	const layout = await board.evaluate((section) => {
		const headers = Array.from(section.querySelectorAll('thead th')).slice(1);
		const table = section.querySelector('table');
		const viewport = section.querySelector<HTMLElement>('[data-timetable-scroll-container]');
		if (!table || !viewport) throw new Error('Timetable layout elements are missing');
		return {
			periodWidths: headers.map((header) => header.getBoundingClientRect().width),
			tableRight: table.getBoundingClientRect().right,
			viewportRight: viewport.getBoundingClientRect().right,
			hasHorizontalOverflow: viewport.scrollWidth > viewport.clientWidth + 1
		};
	});

	expect(layout.hasHorizontalOverflow).toBe(false);
	expect(layout.tableRight).toBeLessThanOrEqual(layout.viewportRight + 1);
	expect(Math.max(...layout.periodWidths) - Math.min(...layout.periodWidths)).toBeLessThanOrEqual(
		1
	);
});

test('keeps ten equal period columns within the whole-school overview', async ({ page }) => {
	await installDenseWorkspace(page);
	await page.goto(wholeSchoolUrl());

	const overview = page.locator('section').filter({
		has: page.getByRole('heading', { name: 'ภาพรวมทั้งโรงเรียน · วันจันทร์' })
	});
	await expect(overview).toBeVisible();
	await expect(overview.getByRole('columnheader')).toHaveCount(11);

	const layout = await overview.evaluate((section) => {
		const headers = Array.from(section.querySelectorAll('thead th')).slice(1);
		const table = section.querySelector('table');
		const viewport = table?.parentElement;
		if (!table || !viewport) throw new Error('Whole-school timetable layout elements are missing');
		return {
			periodWidths: headers.map((header) => header.getBoundingClientRect().width),
			hasHorizontalOverflow: viewport.scrollWidth > viewport.clientWidth + 1
		};
	});

	expect(layout.hasHorizontalOverflow).toBe(false);
	expect(Math.max(...layout.periodWidths) - Math.min(...layout.periodWidths)).toBeLessThanOrEqual(
		1
	);
});

test('keeps the unscheduled lesson tray compact beside the timetable board', async ({ page }) => {
	await installDenseBoard(page);

	const trayWidth = await page
		.getByRole('complementary', { name: 'คาบที่ยังไม่ได้จัด' })
		.evaluate((tray) => tray.getBoundingClientRect().width);
	expect(trayWidth).toBeLessThanOrEqual(250);
});

test('stretches a shorter lesson card to fill the timetable row', async ({ page }) => {
	await installDenseBoard(page);

	const board = page.getByRole('region', { name: 'ตารางของ ม.1/1' });
	const shortCell = board.locator(
		`td[data-timetable-day="MON"][data-timetable-period-id="${timetableIds.period1}"]`
	);
	const shortCard = shortCell.locator(`[data-block-id="${timetableIds.blockA}"]`);
	const [cellBox, cardBox] = await Promise.all([shortCell.boundingBox(), shortCard.boundingBox()]);
	expect(cellBox).not.toBeNull();
	expect(cardBox).not.toBeNull();
	expect(cardBox!.height).toBeGreaterThanOrEqual(cellBox!.height - 14);
});
