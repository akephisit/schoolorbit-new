import { expect, test } from '@playwright/test';

import {
	installTimetableMock,
	makeSynchronizedTimetableBlock,
	makeTimetableBlock,
	timetableIds
} from './timetable-test-harness';

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
	const dayHeaders = board.locator('tbody tr > th');
	await expect(dayHeaders).toHaveText(['จ.', 'อ.', 'พ.', 'พฤ.', 'ศ.']);
	const fullDayNames = ['วันจันทร์', 'วันอังคาร', 'วันพุธ', 'วันพฤหัสบดี', 'วันศุกร์'];
	for (const [index, fullDayName] of fullDayNames.entries()) {
		await expect(dayHeaders.nth(index)).toHaveAttribute('aria-label', fullDayName);
	}
});

test('places exactly one unscheduled period and projects it into both editable views', async ({
	page
}) => {
	const mock = await installTimetableMock(page, { requiredPeriods: 3 });
	await page.goto(timetableUrl());

	await expect(page.getByText('เหลือ 3/3')).toBeVisible();
	expect(mock.workspaceRequestCount()).toBe(1);
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
	expect(mock.workspaceRequestCount()).toBe(1);
	expect(mock.blocks()).toHaveLength(1);

	await page.getByRole('button', { name: 'กลุ่มเรียน' }).click();
	await expect(page.getByRole('button', { name: /ดูรายละเอียด ค21101/ })).toBeVisible();
});

test('preselects the first preferred room and lets the scheduler override it before placement', async ({
	page
}) => {
	const mock = await installTimetableMock(page, {
		requiredPeriods: 1,
		preferredRoomIds: [timetableIds.roomB, timetableIds.room]
	});
	await page.goto(timetableUrl());

	const trayCard = page.locator('aside article').filter({ hasText: 'ค21101' }).first();
	const teacherPicker = trayCard.getByRole('button', { name: /เลือกครู/ });
	const roomPicker = trayCard.getByRole('button', { name: 'เลือกห้องเรียน' });
	await expect(roomPicker).toContainText('LAB-2');
	const [teacherBox, roomBox] = await Promise.all([
		teacherPicker.boundingBox(),
		roomPicker.boundingBox()
	]);
	expect(teacherBox).not.toBeNull();
	expect(roomBox).not.toBeNull();
	expect(teacherBox!.width).toBeCloseTo(roomBox!.width, 1);
	expect(teacherBox!.height).toBeCloseTo(roomBox!.height, 1);
	await roomPicker.click();
	await page.getByRole('option', { name: /MATH-1/ }).click();
	await expect(roomPicker).toContainText('MATH-1');

	await trayCard.locator('button').first().click();
	await page.getByRole('button', { name: 'วางคาบที่นี่' }).first().click();

	await expect(page.locator(`article[data-block-id="${timetableIds.createdBlock}"]`)).toContainText(
		'MATH-1'
	);
	expect(mock.lastCreateBody()).toMatchObject({ roomId: timetableIds.room });
});

test('shows the dragged lesson preview only in the cell currently under the pointer', async ({
	page
}) => {
	await installTimetableMock(page, { requiredPeriods: 1 });
	await page.goto(timetableUrl());

	const board = page.locator('section[aria-label^="ตารางของ "]');
	const firstRow = board.locator('tbody tr').first();
	const secondRow = board.locator('tbody tr').nth(1);
	const firstPeriod = firstRow.locator('td').nth(0);
	const secondPeriod = firstRow.locator('td').nth(1);
	const beforeSecondRow = await secondRow.boundingBox();
	expect(beforeSecondRow).not.toBeNull();

	const trayCard = page.locator('aside article[draggable="true"]').first();
	const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
	await trayCard.dispatchEvent('dragstart', { dataTransfer });
	await firstPeriod.dispatchEvent('dragover', { dataTransfer });

	const preview = page.locator('[data-timetable-placement-preview]');
	await expect(preview).toHaveCount(1);
	await expect(firstPeriod.locator('[data-timetable-placement-preview]')).toContainText('ค21101');
	await expect(firstPeriod.locator('[data-timetable-placement-preview]')).toContainText(
		'คณิตศาสตร์พื้นฐาน'
	);

	await secondPeriod.dispatchEvent('dragover', { dataTransfer });
	await expect(firstPeriod.locator('[data-timetable-placement-preview]')).toHaveCount(0);
	await expect(secondPeriod.locator('[data-timetable-placement-preview]')).toHaveCount(1);
	const afterSecondRow = await secondRow.boundingBox();
	expect(afterSecondRow).not.toBeNull();
	expect(afterSecondRow!.y).toBeCloseTo(beforeSecondRow!.y, 1);

	await trayCard.dispatchEvent('dragend', { dataTransfer });
	await expect(preview).toHaveCount(0);
});

test('places immediately and keeps other timetable cards editable while saves run in background', async ({
	page
}) => {
	const movableBlock = makeTimetableBlock(timetableIds.blockB, timetableIds.period3, {
		groupId: timetableIds.groupB,
		offeringId: timetableIds.offeringB,
		code: 'ว21101',
		name: 'วิทยาศาสตร์พื้นฐาน',
		instructorIds: [timetableIds.teacherB]
	});
	const mock = await installTimetableMock(page, {
		blocks: [movableBlock],
		requiredPeriods: 2,
		previewDelayMs: 600,
		createDelayMs: 500,
		updateDelayMs: 500
	});
	await page.goto(timetableUrl());

	const firstPeriod = page.locator('td[aria-label^="วันจันทร์ คาบ 1"]').first();
	const secondPeriod = page.locator('td[aria-label^="วันจันทร์ คาบ 2"]').first();
	const trayCard = page.locator('aside article').filter({ hasText: 'ค21101' }).first();
	const createTransfer = await page.evaluateHandle(() => new DataTransfer());
	await trayCard.dispatchEvent('dragstart', { dataTransfer: createTransfer });
	await firstPeriod.dispatchEvent('dragover', { dataTransfer: createTransfer });
	await firstPeriod.dispatchEvent('drop', { dataTransfer: createTransfer });
	await trayCard.dispatchEvent('dragend', { dataTransfer: createTransfer });

	await expect(firstPeriod.getByLabel('กำลังบันทึกคาบ คณิตศาสตร์พื้นฐาน')).toBeVisible({
		timeout: 300
	});
	await expect(page.getByText('เหลือ 1/2')).toBeVisible({ timeout: 300 });

	const movableCard = page.locator(`article[data-block-id="${timetableIds.blockB}"]`);
	const moveTransfer = await page.evaluateHandle(() => new DataTransfer());
	await movableCard.dispatchEvent('dragstart', { dataTransfer: moveTransfer });
	await secondPeriod.dispatchEvent('dragover', { dataTransfer: moveTransfer });
	await secondPeriod.dispatchEvent('drop', { dataTransfer: moveTransfer });
	await movableCard.dispatchEvent('dragend', { dataTransfer: moveTransfer });

	await expect(secondPeriod.getByLabel('กำลังบันทึกคาบ วิทยาศาสตร์พื้นฐาน')).toBeVisible({
		timeout: 300
	});
	await expect(page.getByLabel(/^กำลังบันทึกคาบ /)).toHaveCount(0, { timeout: 4000 });
	expect(mock.createRequestCount()).toBe(1);
	expect(mock.updateRequestCount()).toBe(1);
	expect(mock.workspaceRequestCount()).toBe(1);
});

test('saves edited room in the background while another lesson can still be dragged', async ({
	page
}) => {
	const blockA = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const blockB = makeTimetableBlock(timetableIds.blockB, timetableIds.period3, {
		groupId: timetableIds.groupB,
		offeringId: timetableIds.offeringB,
		code: 'ว21101',
		name: 'วิทยาศาสตร์พื้นฐาน',
		instructorIds: [timetableIds.teacherB]
	});
	const mock = await installTimetableMock(page, {
		blocks: [blockA, blockB],
		requiredPeriods: 0,
		updateDelayMs: 700
	});
	await page.goto(timetableUrl());

	await page.getByRole('button', { name: /ดูรายละเอียด ค21101/ }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: 'เลือกห้องเรียน' }).click();
	await page.getByRole('option', { name: /LAB-2/ }).click();
	await dialog.getByRole('button', { name: 'บันทึก' }).click();

	const editedCard = page.locator(`article[data-block-id="${timetableIds.blockA}"]`);
	await expect(dialog).toHaveCount(0, { timeout: 300 });
	await expect(editedCard).toContainText('LAB-2', { timeout: 300 });
	await expect(editedCard.getByLabel('กำลังบันทึกคาบ คณิตศาสตร์พื้นฐาน')).toBeVisible({
		timeout: 300
	});

	const movableCard = page.locator(`article[data-block-id="${timetableIds.blockB}"]`);
	const secondPeriod = page.locator('td[aria-label^="วันจันทร์ คาบ 2"]').first();
	const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
	await movableCard.dispatchEvent('dragstart', { dataTransfer });
	await secondPeriod.dispatchEvent('dragover', { dataTransfer });
	await secondPeriod.dispatchEvent('drop', { dataTransfer });
	await movableCard.dispatchEvent('dragend', { dataTransfer });

	await expect(secondPeriod.getByLabel('กำลังบันทึกคาบ วิทยาศาสตร์พื้นฐาน')).toBeVisible({
		timeout: 300
	});
	await expect(page.getByLabel(/^กำลังบันทึกคาบ /)).toHaveCount(0, { timeout: 4000 });
	expect(mock.updateRequestCount()).toBe(2);
	expect(mock.workspaceRequestCount()).toBe(1);
});

test('restores the previous room when a background detail save fails', async ({ page }) => {
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const mock = await installTimetableMock(page, {
		blocks: [block],
		requiredPeriods: 0,
		updateDelayMs: 500,
		failUpdate: true
	});
	await page.goto(timetableUrl());

	await page.getByRole('button', { name: /ดูรายละเอียด ค21101/ }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: 'เลือกห้องเรียน' }).click();
	await page.getByRole('option', { name: /LAB-2/ }).click();
	await dialog.getByRole('button', { name: 'บันทึก' }).click();

	const card = page.locator(`article[data-block-id="${timetableIds.blockA}"]`);
	await expect(card).toContainText('LAB-2', { timeout: 300 });
	await expect(card).toContainText('MATH-1', { timeout: 3000 });
	await expect(page.getByText(/ข้อมูลคาบเปลี่ยนแปลงแล้ว/)).toBeVisible();
	expect(mock.updateRequestCount()).toBe(1);
	expect(mock.workspaceRequestCount()).toBe(1);
});

test('rolls an optimistic placement back when background validation rejects it', async ({
	page
}) => {
	const mock = await installTimetableMock(page, {
		requiredPeriods: 1,
		blockedPeriodId: timetableIds.period2,
		previewDelayMs: 3000
	});
	await page.goto(timetableUrl());

	const trayCard = page.locator('aside article').filter({ hasText: 'ค21101' }).first();
	const destination = page.locator('td[aria-label^="วันจันทร์ คาบ 2"]').first();
	const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
	await trayCard.dispatchEvent('dragstart', { dataTransfer });
	await destination.dispatchEvent('dragover', { dataTransfer });
	await destination.dispatchEvent('drop', { dataTransfer });

	await expect(destination.getByLabel('กำลังบันทึกคาบ คณิตศาสตร์พื้นฐาน')).toBeVisible({
		timeout: 300
	});
	await expect(trayCard).toHaveCount(0, { timeout: 300 });
	await expect(
		page.getByText('วางคาบไม่ได้: ครูคณิตศาสตร์ A มีคาบสอนอยู่แล้ว', { exact: true })
	).toBeVisible();
	await expect(destination.getByRole('button', { name: /ดูรายละเอียด ค21101/ })).toHaveCount(0);
	await expect(page.getByText('เหลือ 1/1')).toBeVisible();
	expect(mock.createRequestCount()).toBe(0);
});

test('removes immediately and restores the card when the background delete fails', async ({
	page
}) => {
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const mock = await installTimetableMock(page, {
		blocks: [block],
		deleteDelayMs: 600,
		failDelete: true
	});
	await page.goto(timetableUrl());

	const cell = page.locator('td[aria-label^="วันจันทร์ คาบ 1"]').first();
	const card = page.locator(`article[data-block-id="${timetableIds.blockA}"]`);
	await card.getByRole('button', { name: /นำ .* ออกจากตาราง/ }).click();
	await page.getByRole('button', { name: 'ยืนยันนำออก' }).click();

	await expect(page.getByRole('alertdialog')).toHaveCount(0, { timeout: 300 });
	await expect(card).toHaveCount(0, { timeout: 300 });
	await expect(cell.getByLabel('กำลังลบคาบ')).toBeVisible({ timeout: 300 });
	await expect(card).toBeVisible({ timeout: 3000 });
	await expect(cell.getByLabel('กำลังลบคาบ')).toHaveCount(0);
	await expect(page.getByText(/ข้อมูลคาบเปลี่ยนแปลงแล้ว/)).toBeVisible();
	expect(mock.deleteRequestCount()).toBe(1);
	expect(mock.workspaceRequestCount()).toBe(1);
});

test('keeps timetable row geometry stable while drag feedback is active', async ({ page }) => {
	await installTimetableMock(page, { requiredPeriods: 1 });
	await page.goto(timetableUrl());

	const board = page.locator('section[aria-label^="ตารางของ "]');
	const firstRow = board.locator('tbody tr').first();
	const secondRow = board.locator('tbody tr').nth(1);
	const firstPeriod = firstRow.locator('td').first();
	const before = await Promise.all([
		firstRow.boundingBox(),
		secondRow.boundingBox(),
		firstPeriod.boundingBox()
	]);
	expect(before.every(Boolean)).toBe(true);

	const trayCard = page.locator('aside article[draggable="true"]').first();
	const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
	await trayCard.dispatchEvent('dragstart', { dataTransfer });
	await expect(firstPeriod).toHaveAttribute('data-state', 'move');

	const after = await Promise.all([
		firstRow.boundingBox(),
		secondRow.boundingBox(),
		firstPeriod.boundingBox()
	]);
	expect(after.every(Boolean)).toBe(true);
	expect(after[0]!.height).toBeCloseTo(before[0]!.height, 1);
	expect(after[1]!.y).toBeCloseTo(before[1]!.y, 1);
	expect(after[2]!.height).toBeCloseTo(before[2]!.height, 1);
});

test('uses border-only placement feedback without visible status labels', async ({ page }) => {
	await installTimetableMock(page, { requiredPeriods: 1 });
	await page.goto(timetableUrl());

	const firstPeriod = page.locator('td[aria-label^="วันจันทร์ คาบ 1"]').first();
	const trayCard = page.locator('aside article[draggable="true"]').first();
	const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
	await trayCard.dispatchEvent('dragstart', { dataTransfer });
	await expect(firstPeriod).toHaveAttribute('data-state', 'move');

	const feedback = await firstPeriod.evaluate((cell) => {
		const hasVisibleStatusLabel = Array.from(cell.querySelectorAll('*')).some((element) => {
			if (element.textContent?.trim() !== 'วางได้') return false;
			const style = getComputedStyle(element);
			const rect = element.getBoundingClientRect();
			return (
				style.visibility !== 'hidden' &&
				style.display !== 'none' &&
				rect.width > 1 &&
				rect.height > 1
			);
		});
		return {
			hasVisibleStatusLabel,
			boxShadow: getComputedStyle(cell).boxShadow
		};
	});
	expect(feedback.hasVisibleStatusLabel).toBe(false);
	expect(feedback.boxShadow).not.toBe('none');
});

test('keeps a quick drop alive while its placement preview is still loading', async ({ page }) => {
	const mock = await installTimetableMock(page, {
		requiredPeriods: 1,
		previewDelayMs: 150
	});
	await page.goto(timetableUrl());

	const trayCard = page.locator('aside article[draggable="true"]').first();
	const firstPeriod = page.locator('td[aria-label^="วันจันทร์ คาบ 1"]').first();
	const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
	await trayCard.dispatchEvent('dragstart', { dataTransfer });
	await firstPeriod.dispatchEvent('dragover', { dataTransfer });
	await firstPeriod.dispatchEvent('drop', { dataTransfer });

	await expect(page.getByText('บันทึกตำแหน่งคาบแล้ว', { exact: true })).toBeVisible();
	expect(mock.createRequestCount()).toBe(1);
	expect(mock.blocks()).toHaveLength(1);
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
	await destination.dispatchEvent('drop', { dataTransfer });

	await expect(
		page.getByText('วางคาบไม่ได้: ครูคณิตศาสตร์ A มีคาบสอนอยู่แล้ว', { exact: true })
	).toBeVisible();
	expect(mock.previewRequestCount()).toBe(1);
	expect(mock.swapRequestCount()).toBe(0);
	expect(mock.updateRequestCount()).toBe(0);
	expect(mock.blocks()[0]?.bellSchedulePeriodId).toBe(timetableIds.period1);
});

test('anchors the native drag image at the point where the lesson card is grabbed', async ({
	page
}) => {
	await page.addInitScript(() => {
		const nativeSetDragImage = DataTransfer.prototype.setDragImage;
		DataTransfer.prototype.setDragImage = function (image, x, y) {
			(
				window as unknown as {
					__timetableDragImage?: { blockId: string | null; x: number; y: number };
				}
			).__timetableDragImage = {
				blockId: image instanceof HTMLElement ? (image.dataset.blockId ?? null) : null,
				x,
				y
			};
			return nativeSetDragImage.call(this, image, x, y);
		};
	});
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	await installTimetableMock(page, { blocks: [block] });
	await page.goto(timetableUrl());

	const card = page.locator(`article[data-block-id="${timetableIds.blockA}"]`);
	const box = await card.boundingBox();
	expect(box).not.toBeNull();
	const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
	await card.dispatchEvent('dragstart', {
		dataTransfer,
		clientX: box!.x + 24,
		clientY: box!.y + 18
	});

	const dragImage = await page.evaluate(
		() =>
			(
				window as unknown as {
					__timetableDragImage?: { blockId: string | null; x: number; y: number };
				}
			).__timetableDragImage ?? null
	);
	expect(dragImage?.blockId).toBe(timetableIds.blockA);
	expect(dragImage?.x).toBeGreaterThanOrEqual(23);
	expect(dragImage?.x).toBeLessThanOrEqual(25);
	expect(dragImage?.y).toBeGreaterThanOrEqual(17);
	expect(dragImage?.y).toBeLessThanOrEqual(19);
});

test('anchors the synchronized tray drag image at the point where it is grabbed', async ({
	page
}) => {
	await page.addInitScript(() => {
		const nativeSetDragImage = DataTransfer.prototype.setDragImage;
		DataTransfer.prototype.setDragImage = function (image, x, y) {
			(
				window as unknown as {
					__timetableTrayDragImage?: { text: string; x: number; y: number };
				}
			).__timetableTrayDragImage = {
				text: image instanceof HTMLElement ? (image.textContent ?? '') : '',
				x,
				y
			};
			return nativeSetDragImage.call(this, image, x, y);
		};
	});
	await installTimetableMock(page, {
		requiredPeriods: 0,
		includeSynchronizedDemand: true
	});
	await page.goto(timetableUrl());

	const trayCard = page.locator('aside article').filter({ hasText: 'ชุมนุม' });
	const box = await trayCard.boundingBox();
	expect(box).not.toBeNull();
	const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
	await trayCard.dispatchEvent('dragstart', {
		dataTransfer,
		clientX: box!.x + 32,
		clientY: box!.y + 21
	});

	const dragImage = await page.evaluate(
		() =>
			(
				window as unknown as {
					__timetableTrayDragImage?: { text: string; x: number; y: number };
				}
			).__timetableTrayDragImage ?? null
	);
	expect(dragImage?.text).toContain('ชุมนุม');
	expect(dragImage?.x).toBeGreaterThanOrEqual(31);
	expect(dragImage?.x).toBeLessThanOrEqual(33);
	expect(dragImage?.y).toBeGreaterThanOrEqual(20);
	expect(dragImage?.y).toBeLessThanOrEqual(22);
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

test('reserves selected teachers when placing a synchronized activity and edits them later', async ({
	page
}) => {
	const mock = await installTimetableMock(page, {
		requiredPeriods: 0,
		includeSynchronizedDemand: true
	});
	await page.goto(timetableUrl());

	const trayCard = page.locator('aside article').filter({ hasText: 'ชุมนุม' });
	const teacherPicker = trayCard.getByRole('button', { name: 'ยังไม่กำหนดครู' });
	const roomPicker = trayCard.getByRole('button', { name: 'เลือกห้องเรียน' });
	await expect(teacherPicker).toBeVisible();
	const [teacherBox, roomBox] = await Promise.all([
		teacherPicker.boundingBox(),
		roomPicker.boundingBox()
	]);
	expect(teacherBox).not.toBeNull();
	expect(roomBox).not.toBeNull();
	expect(teacherBox!.y).toBeCloseTo(roomBox!.y, 1);
	expect(teacherBox!.height).toBeCloseTo(roomBox!.height, 1);
	await roomPicker.click();
	await page.getByRole('option', { name: /LAB-2/ }).click();
	await teacherPicker.click();
	await page.getByRole('button', { name: 'ครูทุกคน', exact: true }).click();
	await page.keyboard.press('Escape');
	await trayCard.locator('button').first().click();
	await page.getByRole('button', { name: 'วางคาบที่นี่' }).first().click();
	const createdBlockButton = page.getByRole('button', { name: /ดูรายละเอียด ชุมนุม/ });
	await expect(createdBlockButton).toBeVisible();

	expect(mock.synchronizedCreateRequestCount()).toBe(1);
	expect(mock.lastSynchronizedCreateBody()).toMatchObject({
		teacherIds: [timetableIds.teacherA, timetableIds.teacherB],
		roomId: timetableIds.roomB
	});
	await createdBlockButton.click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: 'ครูทุกคน' }).click();
	await page
		.getByRole('button', { name: /ครูคณิตศาสตร์ A/ })
		.last()
		.click();
	await page.keyboard.press('Escape');
	await dialog.getByRole('button', { name: 'บันทึก' }).click();

	await expect(page.getByText('แก้รายละเอียดคาบแล้ว')).toBeVisible();
	expect(mock.lastUpdateBody()).toMatchObject({ teacherIds: [timetableIds.teacherB] });
});

test('keeps synchronized group instructors managed by delivery when editing reservations', async ({
	page
}) => {
	const block = makeSynchronizedTimetableBlock(
		timetableIds.blockA,
		timetableIds.period1,
		[timetableIds.teacherB],
		[timetableIds.teacherB]
	);
	const mock = await installTimetableMock(page, { blocks: [block] });
	await page.goto(timetableUrl());

	await page.getByRole('button', { name: /ดูรายละเอียด ชุมนุม/ }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByText('ครูผู้สอนของคาบนี้')).toHaveCount(0);
	await expect(dialog.getByText('ครูที่กันเวลาไว้ล่วงหน้า')).toBeVisible();
	await dialog.getByRole('button', { name: 'เลือกครู 1 คน' }).click();
	await expect(page.getByRole('button', { name: /ครูคณิตศาสตร์ B/ }).last()).toBeDisabled();
	await page.keyboard.press('Escape');
	await dialog.getByRole('button', { name: 'บันทึก' }).click();
	await expect(page.getByText('แก้รายละเอียดคาบแล้ว')).toBeVisible();

	expect(mock.lastUpdateBody()).toMatchObject({
		instructorIds: null,
		teacherIds: [timetableIds.teacherB],
		roomId: null,
		clearRoom: true
	});
});
