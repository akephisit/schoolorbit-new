import { expect, test } from '@playwright/test';

import { installTimetableMock, makeTimetableBlock, timetableIds } from './timetable-test-harness';

test.use({ serviceWorkers: 'block', viewport: { width: 1920, height: 1080 } });

const activityBlockId = 'c1000000-0000-4000-8000-000000000204';

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

function teacherBoardUrl(): string {
	return (
		`/staff/academic/timetable?academicYearId=${timetableIds.year}` +
		`&academicTermId=${timetableIds.term}` +
		`&timetableVersionId=${timetableIds.draftVersion}` +
		`&view=teacher&ownerId=${timetableIds.teacherA}`
	);
}

async function installDenseWorkspace(page: Parameters<typeof installTimetableMock>[0]) {
	const shortBlock = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const tallBlock = makeTimetableBlock(timetableIds.blockB, timetableIds.period2, {
		groupId: timetableIds.groupB,
		offeringId: timetableIds.offeringB,
		code: 'OTHER-cf1520d73a57-very-long-course-code',
		name: 'วิทยาศาสตร์พื้นฐานและการออกแบบเทคโนโลยีแบบบูรณาการ',
		instructorIds: [timetableIds.teacherA, timetableIds.teacherB]
	});
	tallBlock.groups[0].instructors[0].displayName =
		'คุณครูผู้สอนวิทยาศาสตร์และเทคโนโลยีชื่อยาวสำหรับทดสอบ';
	const activityBlock = makeTimetableBlock(activityBlockId, timetableIds.period3, {
		code: 'ACTIVITY-HOMEROOM',
		name: 'โฮมรูม',
		instructorIds: [timetableIds.teacherA]
	});
	activityBlock.blockKind = 'activity';

	await installTimetableMock(page, {
		blocks: [shortBlock, tallBlock, activityBlock],
		periodCount: 10,
		requiredPeriods: 2
	});
}

async function installDenseBoard(page: Parameters<typeof installTimetableMock>[0]) {
	await installDenseWorkspace(page);
	await page.goto(boardUrl());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
}

test('keeps ten equal period columns within the desktop timetable board', async ({ page }) => {
	await installDenseBoard(page);

	const board = page.getByRole('region', { name: 'ตารางของ ม.1/1' });
	await expect(board).toBeVisible();
	await expect(board.getByRole('heading', { name: 'ม.1/1', exact: true })).toBeVisible();
	await expect(board.getByText('M1-1', { exact: true })).toHaveCount(0);
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
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();

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

for (const theme of ['light', 'dark']) {
	test(`fills the available desktop height with local board scrolling in ${theme} mode`, async ({
		page
	}) => {
		await installDenseBoard(page);
		await page.evaluate(
			(mode) => document.documentElement.classList.toggle('dark', mode === 'dark'),
			theme
		);
		for (const height of [1080, 800]) {
			await page.setViewportSize({ width: 1920, height });
			const geometry = await page
				.locator('[data-testid="timetable-board-ready"]')
				.evaluate((workspace) => {
					const board = workspace.querySelector('section[aria-label^="ตารางของ "]')!;
					const tray = workspace.querySelector('aside')!;
					const shell = workspace.closest('.school-page-shell')!;
					const main = workspace.closest('main')!;
					const padding = Number.parseFloat(getComputedStyle(shell).paddingBottom);
					return {
						boardBottom: board.getBoundingClientRect().bottom,
						trayBottom: tray.getBoundingClientRect().bottom,
						availableBottom: main.getBoundingClientRect().bottom - padding,
						pageOverflow: main.scrollHeight - main.clientHeight
					};
				});
			expect(Math.abs(geometry.boardBottom - geometry.availableBottom)).toBeLessThanOrEqual(2);
			expect(Math.abs(geometry.trayBottom - geometry.boardBottom)).toBeLessThanOrEqual(1);
			expect(geometry.pageOverflow).toBeLessThanOrEqual(1);
		}
		const scroll = page.locator('[data-timetable-scroll-container]');
		expect(await scroll.evaluate((element) => element.scrollHeight > element.clientHeight)).toBe(
			true
		);
		await scroll.evaluate((element) => {
			element.scrollTop = element.scrollHeight;
		});
		await expect(page.getByRole('rowheader', { name: 'วันศุกร์', exact: true })).toBeInViewport();
	});
}

test('shows each demand’s actual class names under its subject, with readable multiple targets', async ({
	page
}) => {
	const secondHomeroom = '81000000-0000-4000-8000-000000000202';
	await installTimetableMock(page, {
		homerooms: [timetableIds.homeroom, secondHomeroom].map((id, index) => ({
			id,
			code: `M3-${index + 1}`,
			name: `ม.3/${index + 1}`,
			gradeLevelId: '81000000-0000-4000-8000-000000000301',
			gradeLevelType: 'secondary',
			gradeLevelYear: 3,
			roomNumber: String(index + 1),
			isActive: true
		})),
		ordinaryDemands: [
			{
				learningGroupId: timetableIds.groupA,
				learningOfferingId: timetableIds.offeringA,
				offeringCode: 'ค23101',
				offeringName: 'คณิตศาสตร์พื้นฐาน',
				requiredPeriods: 3,
				scheduledPeriods: 0,
				remainingPeriods: 3,
				homeroomIds: [secondHomeroom, timetableIds.homeroom],
				eligibleInstructors: [
					{
						teacherId: timetableIds.teacherA,
						displayName: 'ครูคณิตศาสตร์',
						role: 'primary',
						orderIndex: 0
					}
				]
			}
		],
		includeSynchronizedDemand: true
	});
	await page.goto(boardUrl());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	const tray = page.getByRole('complementary', { name: 'คาบที่ยังไม่ได้จัด' });
	const course = tray.locator('article').filter({ hasText: 'ค23101' });
	const targets = course.locator('[data-timetable-demand-targets]');
	await expect(targets).toHaveText('ม.3/2, ม.3/1');
	const [subjectBox, targetsBox] = await Promise.all([
		course.getByText('คณิตศาสตร์พื้นฐาน', { exact: true }).boundingBox(),
		targets.boundingBox()
	]);
	expect(targetsBox!.y).toBeGreaterThanOrEqual(subjectBox!.y + subjectBox!.height);
	await expect(
		tray.locator('article').filter({ hasText: 'ชุมนุม' }).locator('[data-timetable-demand-targets]')
	).toHaveText('ม.3/1');
	await page.setViewportSize({ width: 390, height: 844 });
	await expect(targets).toHaveText('ม.3/2, ม.3/1');
	expect(
		await targets.evaluate((element) => ({
			overflow: getComputedStyle(element).overflow,
			whiteSpace: getComputedStyle(element).whiteSpace
		}))
	).toEqual({ overflow: 'visible', whiteSpace: 'normal' });
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

test('keeps the remove action overlaid without an internal divider row', async ({ page }) => {
	await installDenseBoard(page);

	const card = page.locator(`[data-block-id="${timetableIds.blockA}"]`);
	const removeButton = card.getByRole('button', { name: /นำ .* ออกจากตาราง/ });
	await expect(removeButton).toBeVisible();
	const hasInternalDivider = await removeButton.evaluate((button, cardId) => {
		const cardElement = document.querySelector(`[data-block-id="${cardId}"]`);
		let parent = button.parentElement;
		while (parent && parent !== cardElement) {
			if (Number.parseFloat(getComputedStyle(parent).borderTopWidth) > 0) return true;
			parent = parent.parentElement;
		}
		return false;
	}, timetableIds.blockA);
	expect(hasInternalDivider).toBe(false);
});

test('keeps every lesson field on one line and the card inside its timetable row', async ({
	page
}) => {
	await installDenseBoard(page);

	const board = page.getByRole('region', { name: 'ตารางของ ม.1/1' });
	const tallCell = board.locator(
		`td[data-timetable-day="MON"][data-timetable-period-id="${timetableIds.period2}"]`
	);
	const nextRowCell = board.locator(
		`td[data-timetable-day="TUE"][data-timetable-period-id="${timetableIds.period2}"]`
	);
	const tallCard = tallCell.locator(`[data-block-id="${timetableIds.blockB}"]`);
	const lines = tallCard.locator('[data-timetable-card-line]');

	await expect(lines).toHaveCount(4);
	const lineMetrics = await lines.evaluateAll((elements) =>
		elements.map((element) => {
			const style = getComputedStyle(element);
			return {
				height: element.getBoundingClientRect().height,
				lineHeight: Number.parseFloat(style.lineHeight),
				overflow: style.overflow,
				whiteSpace: style.whiteSpace
			};
		})
	);
	for (const metric of lineMetrics) {
		expect(metric.height).toBeLessThanOrEqual(metric.lineHeight + 1);
		expect(metric.overflow).toBe('hidden');
		expect(metric.whiteSpace).toBe('nowrap');
	}
	await expect(tallCard.getByText(/\+1$/)).toBeVisible();

	const [cellBox, nextRowBox, cardBox] = await Promise.all([
		tallCell.boundingBox(),
		nextRowCell.boundingBox(),
		tallCard.boundingBox()
	]);
	expect(cellBox).not.toBeNull();
	expect(nextRowBox).not.toBeNull();
	expect(cardBox).not.toBeNull();
	expect(cardBox!.y + cardBox!.height).toBeLessThanOrEqual(cellBox!.y + cellBox!.height - 5);
	expect(cardBox!.y + cardBox!.height).toBeLessThanOrEqual(nextRowBox!.y);
});

test('shows only context that adds information for homeroom and teacher views', async ({
	page
}) => {
	await installDenseWorkspace(page);
	await page.goto(boardUrl());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();

	let card = page.locator(`[data-block-id="${timetableIds.blockB}"]`);
	await expect(card).toBeVisible();
	await expect(card.getByText('ม.1/1 วิทยาศาสตร์', { exact: true })).toHaveCount(0);

	await page.goto(teacherBoardUrl());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	card = page.locator(`[data-block-id="${timetableIds.blockB}"]`);
	await expect(card).toBeVisible();
	await expect(card.getByText('ม.1/1', { exact: true })).toBeVisible();
	await expect(card.getByText('ม.1/1 วิทยาศาสตร์', { exact: true })).toHaveCount(0);
});

test('keeps teacher-view card typography compact and inside its period column', async ({
	page
}) => {
	await installDenseWorkspace(page);
	await page.goto(teacherBoardUrl());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();

	const board = page.getByRole('region', { name: 'ตารางของ ครูคณิตศาสตร์' });
	const cell = board.locator(
		`td[data-timetable-day="MON"][data-timetable-period-id="${timetableIds.period2}"]`
	);
	const card = cell.locator(`[data-block-id="${timetableIds.blockB}"]`);
	await expect(card).toBeVisible();

	const [cellBox, cardBox, textMetrics] = await Promise.all([
		cell.boundingBox(),
		card.boundingBox(),
		card.locator('[data-timetable-card-line]').evaluateAll((elements) =>
			elements.map((element) => {
				const style = getComputedStyle(element);
				return {
					fontSize: Number.parseFloat(style.fontSize),
					lineHeight: Number.parseFloat(style.lineHeight)
				};
			})
		)
	]);
	expect(cellBox).not.toBeNull();
	expect(cardBox).not.toBeNull();
	expect(cardBox!.x).toBeGreaterThanOrEqual(cellBox!.x + 5);
	expect(cardBox!.x + cardBox!.width).toBeLessThanOrEqual(cellBox!.x + cellBox!.width - 5);
	expect(Math.max(...textMetrics.map((metric) => metric.fontSize))).toBe(11);
	for (const metric of textMetrics) {
		expect(metric.lineHeight - metric.fontSize).toBeGreaterThanOrEqual(4);
	}
});

test('hides activity codes and uses a neutral card border', async ({ page }) => {
	await installDenseBoard(page);

	const card = page.locator(`[data-block-id="${activityBlockId}"]`);
	await expect(card).toBeVisible();
	await expect(card.getByText('ACTIVITY-HOMEROOM', { exact: true })).toHaveCount(0);
	await expect(card.getByText('โฮมรูม', { exact: true })).toHaveCount(1);
	const borders = await card.evaluate((element) => {
		const style = getComputedStyle(element);
		return {
			leftColor: style.borderLeftColor,
			topColor: style.borderTopColor,
			leftWidth: style.borderLeftWidth,
			topWidth: style.borderTopWidth
		};
	});
	expect(borders.leftColor).toBe(borders.topColor);
	expect(borders.leftWidth).toBe(borders.topWidth);
});

test('hides the redundant teacher row only in teacher view', async ({ page }) => {
	await installDenseWorkspace(page);
	await page.goto(boardUrl());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();

	let card = page.locator(`[data-block-id="${timetableIds.blockA}"]`);
	await expect(card.getByText('ครูคณิตศาสตร์', { exact: true })).toBeVisible();

	await page.goto(teacherBoardUrl());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	card = page.locator(`[data-block-id="${timetableIds.blockA}"]`);
	await expect(card).toBeVisible();
	await expect(card.getByText('ครูคณิตศาสตร์', { exact: true })).toHaveCount(0);
});

test('uses the app font for course codes and drag previews', async ({ page }) => {
	await installTimetableMock(page, {
		blocks: [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)]
	});
	await page.goto(boardUrl());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	const tray = page.getByRole('complementary', { name: 'คาบที่ยังไม่ได้จัด' });
	const code = tray.getByText('ค21101', { exact: true });
	expect(await code.evaluate((element) => getComputedStyle(element).fontFamily)).toContain('Kanit');
	const boardCode = page
		.locator('[data-timetable-lesson-card]')
		.getByText('ค21101', { exact: true });
	expect(await boardCode.evaluate((element) => getComputedStyle(element).fontFamily)).toContain(
		'Kanit'
	);
	const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
	await tray.locator('article').first().dispatchEvent('dragstart', { dataTransfer });
	await page
		.locator(`td[data-timetable-day="MON"][data-timetable-period-id="${timetableIds.period3}"]`)
		.dispatchEvent('dragover', { dataTransfer });
	const previewCode = page
		.locator('[data-timetable-placement-preview]')
		.getByText('ค21101', { exact: true })
		.first();
	await expect(previewCode).toBeVisible();
	expect(await previewCode.evaluate((element) => getComputedStyle(element).fontFamily)).toContain(
		'Kanit'
	);
});

test('bounds a long tray to the board and scrolls all remaining demands', async ({ page }) => {
	const demands = Array.from({ length: 12 }, (_, index) => ({
		learningGroupId: `group-${index}`,
		learningOfferingId: `offering-${index}`,
		offeringCode: `ค211${index}`,
		offeringName: `วิชาทดสอบ ${index + 1}`,
		requiredPeriods: 3,
		scheduledPeriods: 0,
		remainingPeriods: 3,
		homeroomIds: [timetableIds.homeroom],
		eligibleInstructors: [
			{
				teacherId: timetableIds.teacherA,
				displayName: 'ครูคณิตศาสตร์ A',
				role: 'primary',
				orderIndex: 0
			}
		]
	}));
	await installTimetableMock(page, {
		ordinaryDemands: demands,
		learningGroups: demands.map((demand) => ({
			id: demand.learningGroupId,
			learningOfferingId: demand.learningOfferingId,
			code: demand.offeringCode,
			name: demand.offeringName,
			status: 'published',
			rosterStatus: 'published',
			offeringKind: 'course',
			subjectType: 'BASIC',
			offeringCode: demand.offeringCode,
			offeringName: demand.offeringName,
			homeroomIds: demand.homeroomIds,
			preferredRoomIds: [],
			eligibleInstructors: demand.eligibleInstructors
		})),
		includeSynchronizedDemand: true
	});
	await page.goto(boardUrl());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	const tray = page.getByRole('complementary', { name: 'คาบที่ยังไม่ได้จัด' });
	const board = page.getByRole('region', { name: 'ตารางของ ม.1/1' });
	const [trayBox, boardBox] = await Promise.all([tray.boundingBox(), board.boundingBox()]);
	expect(
		Math.abs(trayBox!.y + trayBox!.height - boardBox!.y - boardBox!.height)
	).toBeLessThanOrEqual(1);
	const scroll = tray.locator('[data-timetable-tray-scroll]');
	expect(await scroll.evaluate((element) => element.scrollHeight > element.clientHeight)).toBe(
		true
	);
	await scroll.evaluate((element) => {
		element.scrollTop = element.scrollHeight;
	});
	await expect(tray.getByText('วิชาทดสอบ 12', { exact: true })).toBeInViewport();
	await page.setViewportSize({ width: 390, height: 844 });
	await expect(tray).toBeVisible();
	await expect(board).toBeVisible();
	const mobileBoxes = await Promise.all([tray.boundingBox(), board.boundingBox()]);
	expect(mobileBoxes[1]!.y).toBeGreaterThanOrEqual(mobileBoxes[0]!.y + mobileBoxes[0]!.height);
});

test('hides synchronized demand codes in teacher view', async ({ page }) => {
	await installTimetableMock(page, { includeSynchronizedDemand: true });
	await page.goto(teacherBoardUrl());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	const activity = page.locator('aside article').filter({ hasText: 'ชุมนุม' });
	await expect(activity).toBeVisible();
	await expect(activity.getByText('CLUB', { exact: true })).toHaveCount(0);
	await expect(activity.getByText('พร้อมกัน 1 ห้อง', { exact: true })).toBeVisible();
});
