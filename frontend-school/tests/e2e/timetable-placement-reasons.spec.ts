import { expect, test } from '@playwright/test';
import { installTimetableMock, makeTimetableBlock, timetableIds } from './timetable-test-harness';
import { setTimetableTheme } from './timetable-theme';

test.use({ serviceWorkers: 'block' });

function boardUrl(view: 'homeroom' | 'teacher' | 'learning_group' = 'homeroom'): string {
	const owner =
		view === 'teacher'
			? timetableIds.teacherA
			: view === 'learning_group'
				? timetableIds.groupA
				: timetableIds.homeroom;
	return `/staff/academic/timetable?academicYearId=${timetableIds.year}&academicTermId=${timetableIds.term}&timetableVersionId=${timetableIds.draftVersion}&view=${view}&ownerId=${owner}`;
}

for (const layout of [
	{ width: 1440, height: 1000, theme: 'light' as const },
	{ width: 1440, height: 1000, theme: 'dark' as const },
	{ width: 390, height: 844, theme: 'light' as const },
	{ width: 390, height: 844, theme: 'dark' as const }
]) {
	test(`immediate compact reasons and drag details remain stable at ${layout.width}px ${layout.theme}`, async ({
		page
	}) => {
		await page.setViewportSize(layout);
		let release!: () => void;
		const previewGate = new Promise<void>((resolve) => {
			release = resolve;
		});
		const blocker = makeTimetableBlock(timetableIds.blockB, timetableIds.period1, {
			groupId: timetableIds.groupB,
			offeringId: timetableIds.offeringB,
			code: 'ว21101',
			name: 'วิทยาศาสตร์',
			instructorIds: [timetableIds.teacherB, timetableIds.teacherA]
		});
		const secondBlocker = makeTimetableBlock(
			'c1000000-0000-4000-8000-000000000299',
			timetableIds.period1,
			{
				groupId: timetableIds.groupB,
				name: 'กิจกรรมอ่านหนังสือ',
				instructorIds: [timetableIds.teacherA]
			}
		);
		secondBlocker.blockKind = 'activity';
		const mock = await installTimetableMock(page, {
			blocks: [blocker, secondBlocker],
			previewGate,
			preferredRoomIds: [timetableIds.room]
		});
		await page.goto(boardUrl());
		await setTimetableTheme(page, layout.theme);
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		const cell = page.locator(
			`td[data-timetable-day="MON"][data-timetable-period-id="${timetableIds.period1}"]`
		);
		const before = await cell.boundingBox();
		const tray = page.locator('aside article[draggable="true"]').first();
		const transfer = await page.evaluateHandle(() => new DataTransfer());
		await tray.dispatchEvent('dragstart', { dataTransfer: transfer });
		await expect(cell).toHaveAttribute('data-state', 'blocked');
		const reason = cell.locator('[data-timetable-placement-reason]');
		await expect(reason).toContainText('ครูคณิตศาสตร์ A ไม่ว่าง +1');
		await expect(reason).toContainText('วิชา: ว21101 วิทยาศาสตร์');
		await expect(reason).toContainText('ม.1/1');
		expect(mock.previewRequestCount()).toBe(0);
		const after = await cell.boundingBox();
		expect(after?.height).toBe(before?.height);
		await cell.scrollIntoViewIfNeeded();
		await cell.dispatchEvent('dragover', { dataTransfer: transfer });
		const details = page.getByRole('dialog', { name: 'เหตุผลที่วางคาบไม่ได้' });
		await expect(details).toBeVisible();
		expect(await details.evaluate((element) => getComputedStyle(element).pointerEvents)).toBe(
			'none'
		);
		await expect(details).toContainText('ครูคณิตศาสตร์ A ไม่ว่าง');
		await expect(details).toContainText('ม.1/1 ไม่ว่าง');
		await expect(details).toContainText('ห้องสอน ห้องคณิตศาสตร์ ไม่ว่าง');
		await expect(details).toContainText('กิจกรรม: กิจกรรมอ่านหนังสือ');
		await expect(details.locator('li')).toHaveCount(2);
		await expect(cell.locator('[data-timetable-placement-preview]')).toHaveCount(0);
		expect(mock.workspaceRequestCount()).toBe(1);
		await tray.dispatchEvent('dragend', { dataTransfer: transfer });
		release();
		await expect(page.locator('[data-timetable-placement-reason]')).toHaveCount(0);
		await expect(details).not.toBeVisible();
		expect(mock.createRequestCount()).toBe(0);
	});
}

test('teacher view prioritizes the busy class and retains the blocking subject and actual teacher', async ({
	page
}) => {
	const blocker = makeTimetableBlock(timetableIds.blockB, timetableIds.period1, {
		groupId: timetableIds.groupB,
		code: 'ว21101',
		name: 'วิทยาศาสตร์',
		instructorIds: [timetableIds.teacherB]
	});
	const mock = await installTimetableMock(page, { blocks: [blocker] });
	await page.goto(boardUrl('teacher'));
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await page.locator('aside article').first().getByRole('button').first().click();
	const reason = page
		.locator('td[data-timetable-day="MON"] [data-timetable-placement-reason]')
		.first();
	await expect(reason).toContainText('ม.1/1 ไม่ว่าง');
	await expect(reason).toContainText('วิชา: ว21101 วิทยาศาสตร์');
	await expect(reason).toContainText('ครูคณิตศาสตร์ B');
	await reason.click();
	await expect(page.getByRole('dialog', { name: 'เหตุผลที่วางคาบไม่ได้' })).toContainText(
		'ครูคณิตศาสตร์ B'
	);
	expect(mock.previewRequestCount()).toBe(0);
});

test('rejected drop keeps board geometry stable, shows only a toast and never writes', async ({
	page
}) => {
	const mock = await installTimetableMock(page);
	let release!: () => void;
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	let previewRequests = 0;
	await page.route('**/api/academic/timetable-blocks/placement-preview', async (route) => {
		previewRequests += 1;
		await gate;
		const body = route.request().postDataJSON();
		await route.fulfill({
			json: {
				success: true,
				data: {
					state: 'blocked',
					mutation: null,
					sourceBlockId: null,
					targetBlockId: null,
					targetDayOfWeek: body.targetDayOfWeek,
					targetBellSchedulePeriodId: body.targetBellSchedulePeriodId,
					normalizedCandidate: body.candidate,
					conflicts: [
						{
							code: 'TIMETABLE_TEACHER_CONFLICT',
							conflictType: 'teacher',
							existingBlockId: 'outside-authorized-workspace',
							targetKind: 'teacher',
							targetId: timetableIds.teacherA,
							message: 'ครูมีคาบสอนในตารางอื่นอยู่แล้ว'
						}
					]
				}
			}
		});
	});
	await page.goto(boardUrl());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	const tray = page.locator('aside article[draggable="true"]').first();
	const transfer = await page.evaluateHandle(() => new DataTransfer());
	const cell = page.locator('td[data-timetable-day="MON"]').first();
	await tray.dispatchEvent('dragstart', { dataTransfer: transfer });
	await cell.dispatchEvent('dragover', { dataTransfer: transfer });
	await expect(cell.locator('[data-timetable-placement-preview]')).toBeVisible();
	expect(previewRequests).toBe(0);
	const header = page.getByLabel('บริบทตารางสอน');
	const board = page.locator('section[aria-label^="ตารางของ "]');
	const before = await Promise.all([header.boundingBox(), board.boundingBox(), cell.boundingBox()]);
	expect(before.every(Boolean)).toBe(true);
	await cell.dispatchEvent('drop', { dataTransfer: transfer });
	await expect.poll(() => previewRequests).toBe(1);
	release();
	await expect(page.locator('[data-sonner-toast][data-type="error"]')).toHaveText(
		'วางคาบไม่ได้: ครูมีคาบสอนในตารางอื่นอยู่แล้ว'
	);
	await expect(page.locator('main [role="alert"]')).toHaveCount(0);
	await expect(cell.locator('[data-timetable-placement-preview]')).toHaveCount(0);
	await expect(cell.locator('[data-timetable-lesson-card]')).toHaveCount(0);
	const after = await Promise.all([header.boundingBox(), board.boundingBox(), cell.boundingBox()]);
	for (const [index, initial] of before.entries()) {
		const current = after[index];
		expect(initial).not.toBeNull();
		expect(current).not.toBeNull();
		for (const coordinate of ['x', 'y', 'width', 'height'] as const) {
			expect(current![coordinate]).toBeCloseTo(initial![coordinate], 1);
		}
	}
	expect(previewRequests).toBe(1);
	expect(mock.createRequestCount()).toBe(0);
	expect(mock.swapRequestCount()).toBe(0);
	expect(mock.synchronizedCreateRequestCount()).toBe(0);
	expect(mock.updateRequestCount()).toBe(0);
	expect(mock.blocks()).toHaveLength(0);
	expect(mock.workspaceRequestCount()).toBe(1);
});

test('touch selection opens all blocking details and dismisses without cancelling the placement', async ({
	browser
}) => {
	const context = await browser.newContext({
		viewport: { width: 390, height: 844 },
		hasTouch: true,
		isMobile: true,
		serviceWorkers: 'block'
	});
	const page = await context.newPage();
	try {
		const blocker = makeTimetableBlock(timetableIds.blockB, timetableIds.period1, {
			groupId: timetableIds.groupB,
			name: 'วิทยาศาสตร์',
			instructorIds: [timetableIds.teacherA]
		});
		const mock = await installTimetableMock(page, { blocks: [blocker] });
		await page.goto(boardUrl());
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).tap();
		await page.locator('aside article').first().getByRole('button').first().tap();
		const reason = page.locator('[data-timetable-placement-reason]').first();
		await reason.tap();
		const details = page.getByRole('dialog', { name: 'เหตุผลที่วางคาบไม่ได้' });
		await expect(details).toBeVisible();
		expect(await details.evaluate((element) => getComputedStyle(element).pointerEvents)).toBe(
			'auto'
		);
		await expect(details).toContainText('วิชา: ค21101 วิทยาศาสตร์');
		await expect(details).toContainText('ม.1/1 ไม่ว่าง');
		await page.getByRole('heading', { name: 'ม.1/1', exact: true }).tap();
		await expect(details).not.toBeVisible();
		await expect(reason).toBeVisible();
		expect(mock.previewRequestCount()).toBe(0);
		expect(mock.createRequestCount()).toBe(0);
	} finally {
		await context.close();
	}
});

test('reverse swap blockers appear immediately while the source cell keeps its source state', async ({
	page
}) => {
	const source = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const target = makeTimetableBlock(timetableIds.blockB, timetableIds.period2, {
		groupId: timetableIds.groupB,
		name: 'วิทยาศาสตร์',
		instructorIds: [timetableIds.teacherB]
	});
	const third = makeTimetableBlock('c1000000-0000-4000-8000-000000000298', timetableIds.period1, {
		groupId: timetableIds.groupB,
		name: 'คาบสอนห้องอื่น',
		instructorIds: [timetableIds.teacherB]
	});
	third.groups[0].homeroomIds = [];
	third.groups[0].homeroomNames = [];
	const mock = await installTimetableMock(page, { blocks: [source, target, third] });
	await page.goto(boardUrl());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	const transfer = await page.evaluateHandle(() => new DataTransfer());
	await page
		.locator(`article[data-block-id="${source.id}"]`)
		.dispatchEvent('dragstart', { dataTransfer: transfer });
	const sourceCell = page.locator(
		`td[data-timetable-day="MON"][data-timetable-period-id="${timetableIds.period1}"]`
	);
	const targetCell = page.locator(
		`td[data-timetable-day="MON"][data-timetable-period-id="${timetableIds.period2}"]`
	);
	await expect(sourceCell).toHaveAttribute('data-state', 'dragging');
	await expect(sourceCell.locator('[data-timetable-placement-reason]')).toHaveCount(0);
	await expect(targetCell).toHaveAttribute('data-state', 'blocked');
	await expect(targetCell.locator('[data-timetable-placement-reason]')).toContainText(
		'คาบที่สลับกลับ: ครูคณิตศาสตร์ B ไม่ว่าง'
	);
	await expect(targetCell.locator('[data-timetable-placement-reason]')).toContainText(
		'คาบสอนห้องอื่น'
	);
	expect(mock.previewRequestCount()).toBe(0);
});

test('keyboard opens blocking details and Escape dismisses them without a validation request', async ({
	page
}) => {
	const blocker = makeTimetableBlock(timetableIds.blockB, timetableIds.period1, {
		groupId: timetableIds.groupB,
		name: 'วิทยาศาสตร์',
		instructorIds: [timetableIds.teacherA]
	});
	const mock = await installTimetableMock(page, { blocks: [blocker] });
	await page.goto(boardUrl('learning_group'));
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await page.locator('aside article').first().getByRole('button').first().click();
	const reason = page.locator('[data-timetable-placement-reason]').first();
	await expect(reason).toContainText('ครูคณิตศาสตร์ A ไม่ว่าง');
	await reason.focus();
	await page.keyboard.press('Enter');
	const details = page.getByRole('dialog', { name: 'เหตุผลที่วางคาบไม่ได้' });
	await expect(details).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(details).not.toBeVisible();
	expect(mock.previewRequestCount()).toBe(0);
	expect(mock.createRequestCount()).toBe(0);
});

test('native drag passes through the conflict popup to an adjacent valid cell and drops once', async ({
	page
}) => {
	await page.setViewportSize({ width: 1920, height: 1080 });
	const blocker = makeTimetableBlock(timetableIds.blockB, timetableIds.period1, {
		groupId: timetableIds.groupB,
		name: 'วิทยาศาสตร์',
		instructorIds: [timetableIds.teacherA]
	});
	blocker.dayOfWeek = 'TUE';
	const mock = await installTimetableMock(page, {
		blocks: [blocker],
		preferredRoomIds: [timetableIds.room]
	});
	await page.goto(boardUrl());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	const tray = page.locator('aside article[draggable="true"]').first();
	const blocked = page.locator(
		`td[data-timetable-day="TUE"][data-timetable-period-id="${timetableIds.period1}"]`
	);
	const valid = page.locator(
		`td[data-timetable-day="MON"][data-timetable-period-id="${timetableIds.period1}"]`
	);
	const origin = await tray.boundingBox();
	const target = await blocked.boundingBox();
	expect(origin).not.toBeNull();
	expect(target).not.toBeNull();
	await page.mouse.move(origin!.x + 24, origin!.y + 24);
	await page.mouse.down();
	await page.mouse.move(target!.x + target!.width / 2, target!.y + target!.height / 2, {
		steps: 20
	});
	await page.mouse.move(target!.x + target!.width / 2 + 1, target!.y + target!.height / 2);
	const popup = page.getByRole('dialog', { name: 'เหตุผลที่วางคาบไม่ได้' });
	await expect(popup).toBeVisible();
	expect(await popup.evaluate((element) => getComputedStyle(element).pointerEvents)).toBe('none');
	const popupBox = await popup.boundingBox();
	const validBox = await valid.boundingBox();
	expect(popupBox).not.toBeNull();
	expect(validBox).not.toBeNull();
	const overlap = {
		left: Math.max(popupBox!.x, validBox!.x),
		top: Math.max(popupBox!.y, validBox!.y),
		right: Math.min(popupBox!.x + popupBox!.width, validBox!.x + validBox!.width),
		bottom: Math.min(popupBox!.y + popupBox!.height, validBox!.y + validBox!.height)
	};
	expect(overlap.right - overlap.left).toBeGreaterThan(10);
	expect(overlap.bottom - overlap.top).toBeGreaterThan(10);
	const point = { x: (overlap.left + overlap.right) / 2, y: (overlap.top + overlap.bottom) / 2 };
	const hit = await valid.evaluate((cell, point) => {
		const element = document.elementFromPoint(point.x, point.y);
		return { withinCell: cell.contains(element), description: element?.outerHTML.slice(0, 1200) };
	}, point);
	expect(hit.withinCell, hit.description).toBe(true);

	expect(mock.previewRequestCount()).toBe(0);
	await page.mouse.move(point.x, point.y, { steps: 10 });
	await page.mouse.move(point.x + 1, point.y + 1);
	await expect(valid.locator('[data-timetable-placement-preview]')).toBeVisible();
	await page.mouse.up();
	await expect(valid.getByRole('button', { name: /ดูรายละเอียด ค21101/ })).toBeVisible();
	expect(mock.previewRequestCount()).toBe(1);
	expect(mock.createRequestCount()).toBe(1);
	expect(mock.blocks().find((block) => block.id === timetableIds.createdBlock)?.dayOfWeek).toBe(
		'MON'
	);
});
