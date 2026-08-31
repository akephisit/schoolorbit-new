import { expect, test, type Locator } from '@playwright/test';

import { installTimetableMock, makeTimetableEntry, timetableIds } from './timetable-test-harness';

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

async function selectPeriod(dialog: Locator, periodName: string): Promise<void> {
	await dialog.getByRole('button', { name: 'คาบ 1', exact: true }).click();
	await dialog
		.page()
		.getByRole('option', { name: new RegExp(periodName) })
		.click();
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
	await expect(firstPeriod).toHaveAttribute('data-state', 'move');
	await firstPeriod.dispatchEvent('drop', { dataTransfer });
	await trayCard.dispatchEvent('dragend', { dataTransfer });

	await expect(page.getByText('เหลือ 2/3')).toBeVisible();
	expect(mock.previewRequestCount()).toBe(1);
	expect(mock.createRequestCount()).toBe(1);
	expect(mock.entries()).toHaveLength(1);

	await page.getByRole('button', { name: 'กลุ่มเรียน' }).click();
	await expect(page.getByRole('button', { name: /ดูรายละเอียด ค21101/ })).toBeVisible();
});

test('swaps occupied periods through the keyboard-friendly move dialog', async ({ page }) => {
	const entryA = makeTimetableEntry(timetableIds.entryA, timetableIds.period1);
	const entryB = makeTimetableEntry(timetableIds.entryB, timetableIds.period2, {
		groupId: timetableIds.groupB,
		offeringId: timetableIds.offeringB,
		code: 'ว21101',
		name: 'วิทยาศาสตร์พื้นฐาน',
		instructorIds: [timetableIds.teacherB]
	});
	const mock = await installTimetableMock(page, { entries: [entryA, entryB] });
	await page.goto(timetableUrl());

	await page
		.locator(`article[data-entry-id="${timetableIds.entryA}"]`)
		.getByRole('button', { name: 'ย้ายคาบ' })
		.click();
	const dialog = page.getByRole('dialog');
	await selectPeriod(dialog, 'คาบ 2');
	await dialog.getByRole('button', { name: 'ย้ายคาบ' }).click();

	await expect(page.getByText('สลับคาบแล้ว')).toBeVisible();
	expect(mock.previewRequestCount()).toBe(1);
	expect(mock.swapRequestCount()).toBe(1);
	expect(mock.updateRequestCount()).toBe(0);
	const movedEntry = mock.entries().find((entry) => entry.id === timetableIds.entryA);
	expect(movedEntry?.bellSchedulePeriodId).toBe(timetableIds.period2);
});

test('keeps a blocked placement unchanged and explains the teacher conflict', async ({ page }) => {
	const entry = makeTimetableEntry(timetableIds.entryA, timetableIds.period1);
	const mock = await installTimetableMock(page, {
		entries: [entry],
		blockedPeriodId: timetableIds.period3
	});
	await page.goto(timetableUrl());

	await page
		.locator(`article[data-entry-id="${timetableIds.entryA}"]`)
		.getByRole('button', { name: 'ย้ายคาบ' })
		.click();
	const dialog = page.getByRole('dialog');
	await selectPeriod(dialog, 'คาบ 3');
	await dialog.getByRole('button', { name: 'ย้ายคาบ' }).click();

	await expect(page.getByText('ครูคณิตศาสตร์ A มีคาบสอนอยู่แล้ว')).toBeVisible();
	expect(mock.previewRequestCount()).toBe(1);
	expect(mock.swapRequestCount()).toBe(0);
	expect(mock.updateRequestCount()).toBe(0);
	expect(mock.entries()[0]?.bellSchedulePeriodId).toBe(timetableIds.period1);
});

test('requires an exact teacher choice when a group has several eligible teachers', async ({
	page
}) => {
	const mock = await installTimetableMock(page, {
		eligibleInstructorIds: [timetableIds.teacherA, timetableIds.teacherB]
	});
	await page.goto(timetableUrl());

	await page.getByRole('button', { name: 'เลือกครูและคาบ' }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('button', { name: 'เริ่มวาง 1 คาบ' })).toBeDisabled();
	await dialog.getByRole('button', { name: /ครูคณิตศาสตร์ A/ }).click();
	await expect(dialog.getByRole('button', { name: 'เริ่มวาง 1 คาบ' })).toBeEnabled();
	await dialog.getByRole('button', { name: 'เริ่มวาง 1 คาบ' }).click();
	await page.getByRole('button', { name: 'วางคาบที่นี่' }).first().click();
	await expect(page.getByText('เหลือ 2/3')).toBeVisible();

	expect(mock.createRequestCount()).toBe(1);
	expect(mock.entries()[0]?.instructors.map((teacher) => teacher.userId)).toEqual([
		timetableIds.teacherA
	]);
});
