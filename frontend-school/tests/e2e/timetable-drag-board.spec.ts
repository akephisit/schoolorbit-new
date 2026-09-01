import { expect, test } from '@playwright/test';

import { installTimetableMock, makeTimetableBlock, timetableIds } from './timetable-test-harness';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

function timetableUrl(): string {
	return (
		`/staff/academic/timetable?academicYearId=${timetableIds.year}` +
		`&academicTermId=${timetableIds.term}` +
		`&timetableVersionId=${timetableIds.draftVersion}` +
		`&view=homeroom&ownerId=${timetableIds.homeroom}`
	);
}

test('shows periods across the top and weekdays down the left side', async ({ page }) => {
	await installTimetableMock(page);
	await page.goto(timetableUrl());

	const board = page.locator('section[aria-label^="ตารางของ "]');
	await expect(board.locator('thead th').first()).toHaveText('วัน / คาบ');
	await expect(board.locator('thead th').nth(1)).toContainText('คาบ 1');
	await expect(board.locator('thead th').nth(2)).toContainText('คาบ 2');
	await expect(board.locator('tbody tr > th')).toHaveText([
		'วันจันทร์',
		'วันอังคาร',
		'วันพุธ',
		'วันพฤหัสบดี',
		'วันศุกร์'
	]);
});

test('places exactly one unscheduled period and projects it into both editable views', async ({
	page
}) => {
	const mock = await installTimetableMock(page, { requiredPeriods: 3 });
	await page.goto(timetableUrl());

	await expect(page.getByText('เหลือ 3/3')).toBeVisible();
	const trayCard = page.locator('aside article[draggable="true"]').first();
	const firstPeriod = page.locator('td[aria-label^="วันจันทร์ คาบ 1"]').first();
	const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
	await trayCard.dispatchEvent('dragstart', { dataTransfer });
	await firstPeriod.dispatchEvent('dragover', { dataTransfer });
	await expect(firstPeriod).toHaveAttribute('data-state', 'move');
	await firstPeriod.dispatchEvent('drop', { dataTransfer });
	await trayCard.dispatchEvent('dragend', { dataTransfer });

	await expect(page.getByText('เหลือ 2/3')).toBeVisible();
	expect(mock.previewRequestCount()).toBe(1);
	expect(mock.createRequestCount()).toBe(1);
	expect(mock.blocks()).toHaveLength(1);

	await page.getByRole('button', { name: 'กลุ่มเรียน' }).click();
	await expect(page.getByRole('button', { name: /ดูรายละเอียด ค21101/ })).toBeVisible();
});

test('swaps occupied periods by dragging one block onto the other', async ({ page }) => {
	const blockA = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const blockB = makeTimetableBlock(timetableIds.blockB, timetableIds.period2, {
		groupId: timetableIds.groupB,
		offeringId: timetableIds.offeringB,
		code: 'ว21101',
		name: 'วิทยาศาสตร์พื้นฐาน',
		instructorIds: [timetableIds.teacherB]
	});
	const mock = await installTimetableMock(page, { blocks: [blockA, blockB] });
	await page.goto(timetableUrl());

	const source = page.locator(`article[data-block-id="${timetableIds.blockA}"]`);
	const destination = page
		.locator(`td[data-timetable-period-id="${timetableIds.period2}"]`)
		.first();
	const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
	await source.dispatchEvent('dragstart', { dataTransfer });
	await destination.dispatchEvent('dragover', { dataTransfer });
	await expect(destination).toHaveAttribute('data-state', 'swap');
	await destination.dispatchEvent('drop', { dataTransfer });
	await source.dispatchEvent('dragend', { dataTransfer });

	await expect(page.getByText('บันทึกตำแหน่งคาบแล้ว')).toBeVisible();
	expect(mock.previewRequestCount()).toBe(1);
	expect(mock.swapRequestCount()).toBe(1);
	expect(mock.updateRequestCount()).toBe(0);
	const movedBlock = mock.blocks().find((block) => block.id === timetableIds.blockA);
	expect(movedBlock?.bellSchedulePeriodId).toBe(timetableIds.period2);
});

test('keeps a blocked placement unchanged and explains the teacher conflict', async ({ page }) => {
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const mock = await installTimetableMock(page, {
		blocks: [block],
		blockedPeriodId: timetableIds.period3
	});
	await page.goto(timetableUrl());

	const source = page.locator(`article[data-block-id="${timetableIds.blockA}"]`);
	const destination = page
		.locator(`td[data-timetable-period-id="${timetableIds.period3}"]`)
		.first();
	const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
	await source.dispatchEvent('dragstart', { dataTransfer });
	await destination.dispatchEvent('dragover', { dataTransfer });

	await expect(page.getByText('ครูคณิตศาสตร์ A มีคาบสอนอยู่แล้ว')).toBeVisible();
	expect(mock.previewRequestCount()).toBe(1);
	expect(mock.swapRequestCount()).toBe(0);
	expect(mock.updateRequestCount()).toBe(0);
	expect(mock.blocks()[0]?.bellSchedulePeriodId).toBe(timetableIds.period1);
});

test('requires an exact teacher choice when a group has several eligible teachers', async ({
	page
}) => {
	const mock = await installTimetableMock(page, {
		eligibleInstructorIds: [timetableIds.teacherA, timetableIds.teacherB]
	});
	await page.goto(timetableUrl());

	const trayCard = page.locator('aside article').filter({ hasText: 'ค21101' }).first();
	await trayCard.getByRole('button', { name: /เลือกครู/ }).click();
	await page.getByRole('button', { name: /ครูคณิตศาสตร์ A/ }).click();
	await expect(trayCard).toHaveAttribute('draggable', 'false');
	await page.getByRole('button', { name: /ครูคณิตศาสตร์ B/ }).click();
	await expect(trayCard).toHaveAttribute('draggable', 'true');
	await page.keyboard.press('Escape');
	await trayCard.locator('button').first().click();
	await page.getByRole('button', { name: 'วางคาบที่นี่' }).first().click();
	await expect(page.getByText('เหลือ 2/3')).toBeVisible();

	expect(mock.createRequestCount()).toBe(1);
	expect(mock.blocks()[0]?.groups[0]?.instructors.map((teacher) => teacher.teacherId)).toEqual([
		timetableIds.teacherB
	]);
});
