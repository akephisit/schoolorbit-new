import { sessionState } from './fixtures/session-state';
import { expect, test, type Page, type Route } from '@playwright/test';

import {
	makeStructuralTimetableBlock,
	makeSynchronizedTimetableBlock,
	makeTimetableBlock,
	timetableIds
} from './timetable-test-harness';

test.use({ serviceWorkers: 'block', hasTouch: true });

const staffId = timetableIds.teacherA;
const academicYearId = '20000000-0000-4000-8000-000000000001';
const academicTermId = '30000000-0000-4000-8000-000000000001';
const configuredPeriodEntries = [
	makeStructuralTimetableBlock(timetableIds.blockA, timetableIds.period1, 'ช่วงเตรียมการ 1', [
		timetableIds.teacherA
	]),
	makeStructuralTimetableBlock(timetableIds.blockB, timetableIds.period2, 'ช่วงเตรียมการ 2', [
		timetableIds.teacherA
	]),
	makeTimetableBlock(timetableIds.createdBlock, timetableIds.period3)
];

function fulfillJson(route: Route, data: unknown) {
	return route.fulfill({
		status: 200,
		contentType: 'application/json',
		body: JSON.stringify({ success: true, data })
	});
}

async function mockStaffTimetableApis(
	page: Page,
	items: unknown[],
	schoolDays = ['MON', 'TUE', 'WED', 'THU', 'FRI']
) {
	let timetableReads = 0;
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			if (url.pathname === '/api/auth/me') {
				await fulfillJson(route, {
					id: staffId,
					username: 'teacher1',
					firstName: 'สายใจ',
					displayName: 'สายใจ วิทยา',
					lastName: 'วิทยา',
					userType: 'staff',
					status: 'ACTIVE',
					session: sessionState(),
					createdAt: '2026-08-07T00:00:00Z',
					email: null,
					nationalId: null,
					phone: null,
					profileImageFileId: null,
					permissions: []
				});
				return;
			}

			if (url.pathname === '/api/academic/context/options') {
				await fulfillJson(route, {
					years: [
						{
							isVisible: true,
							id: academicYearId,
							name: 'ปีการศึกษา 2569',
							year: 2569,
							status: 'active'
						}
					],
					terms: [
						{
							isVisible: true,
							id: academicTermId,
							academicYearId,
							name: 'ภาคเรียนที่ 1',
							code: '1',
							sequence: 1,
							termType: 'regular',
							schoolDays,
							status: 'active',
							includedInYearResult: true,
							blocksYearClosure: true
						}
					],
					defaultAcademicYearId: academicYearId,
					defaultAcademicTermId: academicTermId
				});
				return;
			}

			if (url.pathname === '/api/me/timetable') {
				timetableReads += 1;
				await fulfillJson(route, items);
				return;
			}

			if (url.pathname === '/api/notifications/stream') {
				await route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
				return;
			}

			if (url.pathname === '/api/school/settings') {
				await route.fulfill({
					status: 403,
					contentType: 'application/json',
					body: JSON.stringify({ success: false, error: 'forbidden' })
				});
				return;
			}

			if (url.pathname === '/api/menu/user') {
				await fulfillJson(route, { groups: [] });
				return;
			}

			if (url.pathname === '/api/me/work-items/counts') {
				await fulfillJson(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				});
				return;
			}

			if (url.pathname === '/api/notifications') {
				await fulfillJson(route, { items: [], unread_count: 0 });
				return;
			}

			if (url.pathname === '/api/school/public') {
				await fulfillJson(route, { schoolName: 'ซับน้อยเหนือวิทยาคม' });
				return;
			}

			await fulfillJson(route, {});
		}
	);
	return () => timetableReads;
}

async function expectConfiguredGrid(page: Page) {
	await expect(page.getByRole('columnheader', { name: /คาบ 1/ })).toBeVisible();
	await expect(page.getByRole('columnheader', { name: /คาบ 2/ })).toBeVisible();
	await expect(page.getByRole('columnheader', { name: /คาบ 3/ })).toBeVisible();
	await expect(
		page.getByTestId('own-timetable-week').getByText('จันทร์', { exact: true })
	).toBeVisible();
	await expect(page.getByText('เสาร์', { exact: true })).toHaveCount(0);
	await expect(page.getByText('อาทิตย์', { exact: true })).toHaveCount(0);
}

test('shows configured periods that precede the teacher first lesson', async ({ page }) => {
	await mockStaffTimetableApis(page, configuredPeriodEntries);

	await page.goto('/staff/timetable');

	await expectConfiguredGrid(page);
	await expect(page.getByTestId('own-timetable-week').getByText('ค21101')).toBeVisible();
});

test('shows an explicit empty state when the teacher has no lessons', async ({ page }) => {
	await mockStaffTimetableApis(page, []);

	await page.goto('/staff/timetable');

	await expect(page.getByText('ยังไม่มีตารางสอน', { exact: true })).toBeVisible();
	await expect(page.getByText('ยังไม่มีคาบสอนของคุณในภาคเรียนนี้')).toBeVisible();
});

test('shows complete multiline shared cards without codes teachers or shared labels', async ({
	page
}) => {
	const structuralTitle = 'ประชุมครู\nประจำเดือน';
	const synchronizedTitle = 'ชุมนุม\nถ่ายภาพ';
	const structuralBlock = makeStructuralTimetableBlock(
		timetableIds.blockA,
		timetableIds.period1,
		structuralTitle,
		[timetableIds.teacherA]
	);
	const synchronizedBlock = {
		...makeSynchronizedTimetableBlock(timetableIds.blockB, timetableIds.period2, [
			timetableIds.teacherB
		]),
		offeringName: synchronizedTitle
	};
	const courseBlock = makeTimetableBlock(timetableIds.createdBlock, timetableIds.period3);
	await mockStaffTimetableApis(page, [structuralBlock, synchronizedBlock, courseBlock]);

	await page.goto('/staff/timetable');

	const week = page.getByTestId('own-timetable-week');
	const structuralTitleElement = week.getByText(structuralTitle, { exact: true });
	await expect(structuralTitleElement).toBeVisible();
	expect(
		await structuralTitleElement.evaluate((element) => ({
			whiteSpace: getComputedStyle(element).whiteSpace,
			lineClamp: getComputedStyle(element).webkitLineClamp
		}))
	).toEqual({ whiteSpace: 'pre-line', lineClamp: 'none' });

	const synchronizedTitleElement = week.getByText(synchronizedTitle, { exact: true });
	await expect(synchronizedTitleElement).toBeVisible();
	expect(
		await synchronizedTitleElement.evaluate((element) => ({
			whiteSpace: getComputedStyle(element).whiteSpace,
			lineClamp: getComputedStyle(element).webkitLineClamp
		}))
	).toEqual({ whiteSpace: 'pre-line', lineClamp: 'none' });

	const structuralCard = structuralTitleElement.locator('..');
	const structuralCell = structuralCard.locator('../..');
	const [cardBox, cellBox] = await Promise.all([
		structuralCard.boundingBox(),
		structuralCell.boundingBox()
	]);
	expect(cardBox).not.toBeNull();
	expect(cellBox).not.toBeNull();
	const edgeGaps = [
		cardBox!.x - cellBox!.x,
		cellBox!.x + cellBox!.width - cardBox!.x - cardBox!.width,
		cardBox!.y - cellBox!.y,
		cellBox!.y + cellBox!.height - cardBox!.y - cardBox!.height
	];
	expect(Math.max(...edgeGaps) - Math.min(...edgeGaps)).toBeLessThanOrEqual(2);

	await expect(page.getByText('CLUB', { exact: true })).toHaveCount(0);
	await expect(page.getByText('กิจกรรมรวม', { exact: true })).toHaveCount(0);
	await expect(page.getByText('กิจกรรมพร้อมกัน', { exact: true })).toHaveCount(0);
	await expect(page.getByText('ครูคณิตศาสตร์ A', { exact: true })).toHaveCount(0);
	await expect(page.getByText('ครูคณิตศาสตร์ B', { exact: true })).toHaveCount(0);

	await expect(week.getByText('ค21101', { exact: true })).toBeVisible();
	await expect(week.getByText('คณิตศาสตร์พื้นฐาน', { exact: true })).toBeVisible();
});

const longSubject = 'วิทยาศาสตร์และเทคโนโลยีเพื่อการเรียนรู้และการพัฒนาชุมชนอย่างยั่งยืน';
const longGroup = 'กิจกรรม · กลุ่มการเรียนรู้บูรณาการห้องมัธยมศึกษาปีที่หนึ่งกลุ่มพิเศษ';
const longStructural =
	'โฮมรูม\nป้องกันการทุจริต\nส่งเสริมความเป็นพลเมืองดี\nและการเรียนรู้ร่วมกัน\nของนักเรียนทุกคน';

function workspaceEntries() {
	const entries = Array.from({ length: 9 }, (_, index) => ({
		...makeTimetableBlock(`course-${index}`, `period-${index}`, {
			code: `ว2110${index}`,
			name: index === 0 ? longSubject : `วิชาบูรณาการ ${index + 1}`
		}),
		periodName: `คาบ ${index + 1}`,
		startTime: `${String(8 + index).padStart(2, '0')}:00:00`,
		endTime: `${String(9 + index).padStart(2, '0')}:00:00`
	}));
	entries[0].groups = entries[0].groups.map((group) => ({
		...group,
		name: longGroup,
		homeroomNames: ['ม.4/1']
	}));
	return [
		...entries.filter((_, index) => index !== 2 && index !== 4),
		{
			...makeStructuralTimetableBlock('long-structural', 'period-2', longStructural, []),
			periodName: entries[2].periodName,
			startTime: entries[2].startTime,
			endTime: entries[2].endTime
		},
		{ ...entries[0], id: 'collision', offeringCode: 'คาบร่วม', offeringName: 'คาบที่จัดพร้อมกัน' },
		{
			...entries[6],
			id: 'independent-activity',
			blockKind: 'activity',
			schedulingMode: 'independent',
			offeringName: 'กิจกรรมเลือกเสรี'
		},
		{ ...entries[1], id: 'tuesday', dayOfWeek: 'TUE', offeringName: 'วิชาวันอังคาร' },
		{ ...entries[2], id: 'saturday', dayOfWeek: 'SAT', offeringName: 'วิชาวันเสาร์' },
		{ ...entries[3], id: 'sunday', dayOfWeek: 'SUN', offeringName: 'วิชาวันอาทิตย์' },
		{
			...makeStructuralTimetableBlock('shared', 'period-4', 'พักกลางวัน\nรับประทานอาหาร', []),
			periodName: entries[4].periodName,
			startTime: entries[4].startTime,
			endTime: entries[4].endTime,
			dayOfWeek: 'TUE'
		},
		{
			...makeSynchronizedTimetableBlock('activity', 'period-5', []),
			periodName: entries[5].periodName,
			startTime: entries[5].startTime,
			endTime: entries[5].endTime,
			dayOfWeek: 'TUE',
			offeringName: 'กิจกรรมพัฒนาผู้เรียน'
		}
	];
}

async function expectFullText(element: ReturnType<Page['locator']>) {
	const geometry = await element.evaluate((node) => ({
		height: node.clientHeight,
		fullHeight: node.scrollHeight,
		width: node.clientWidth,
		fullWidth: node.scrollWidth,
		clamp: getComputedStyle(node).webkitLineClamp
	}));
	expect(geometry.fullHeight).toBeLessThanOrEqual(geometry.height + 1);
	expect(geometry.fullWidth).toBeLessThanOrEqual(geometry.width + 1);
	expect(geometry.clamp).toBe('none');
}
async function swipePane(page: Page, pane: ReturnType<Page['locator']>, horizontal = false) {
	const box = (await pane.boundingBox())!;
	const session = await page.context().newCDPSession(page);
	const x = box.x + box.width / 2,
		y = box.y + box.height / 2;
	const distance = Math.min(120, (horizontal ? box.width : box.height) / 3);
	try {
		await session.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] });
		for (let step = 1; step <= 12; step++) {
			await session.send('Input.dispatchTouchEvent', {
				type: 'touchMove',
				touchPoints: [
					{
						x: x - (horizontal ? (distance * step) / 12 : 0),
						y: y - (horizontal ? 0 : (distance * step) / 12)
					}
				]
			});
			await page.evaluate(() => new Promise(requestAnimationFrame));
		}
		await session.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
	} finally {
		await session.detach();
	}
}
async function expectNoOuterOverflow(page: Page) {
	const geometry = await page.locator('main').evaluate((main) => ({
		width: main.clientWidth,
		fullWidth: main.scrollWidth,
		height: main.clientHeight,
		fullHeight: main.scrollHeight,
		top: main.scrollTop,
		docWidth: document.documentElement.clientWidth,
		docFullWidth: document.documentElement.scrollWidth
	}));
	expect(geometry.fullWidth).toBeLessThanOrEqual(geometry.width + 1);
	expect(geometry.fullHeight).toBeLessThanOrEqual(geometry.height + 1);
	expect(geometry.docFullWidth).toBeLessThanOrEqual(geometry.docWidth + 1);
	expect(geometry.top).toBe(0);
}
for (const width of [390, 614, 1029, 1533]) {
	for (const theme of ['light', 'dark'] as const) {
		test(`shows one compact weekly grid at ${width}px in ${theme}`, async ({ page }, testInfo) => {
			await page.setViewportSize({ width, height: 888 });
			await page.addInitScript(
				(mode) => localStorage.setItem('ui-preferences', JSON.stringify({ theme: mode })),
				theme
			);
			const reads = await mockStaffTimetableApis(page, workspaceEntries(), [
				'MON',
				'TUE',
				'WED',
				'THU',
				'FRI',
				'SAT',
				'SUN'
			]);
			await page.goto('/staff/timetable');
			await expect(page.getByTestId('personal-timetable-ready')).toBeVisible();
			await expect
				.poll(() => page.locator('html').evaluate((node) => node.classList.contains('dark')))
				.toBe(theme === 'dark');

			const weekly = page.getByRole('table', { name: 'ตารางสอนรายสัปดาห์' });
			const scroll = page.getByRole('region', { name: 'ตารางสอนรายสัปดาห์ เลื่อนเพื่อดูทุกคาบ' });
			await expect(weekly).toHaveCount(1);
			await expect(weekly).toBeVisible();
			await expect(weekly.getByRole('rowheader')).toHaveText([
				'จันทร์',
				'อังคาร',
				'พุธ',
				'พฤหัสบดี',
				'ศุกร์',
				'เสาร์',
				'อาทิตย์'
			]);
			await expect(weekly.getByRole('columnheader')).toHaveCount(10);
			await expect(weekly.getByRole('columnheader', { name: /คาบ 9.*16:00–17:00/ })).toBeVisible();
			await expect(page.getByRole('radiogroup', { name: 'มุมมองตารางสอน' })).toHaveCount(0);
			await expect(page.getByRole('radiogroup', { name: 'วันในตารางสอน' })).toHaveCount(0);
			await expect(page.getByTestId('own-timetable-details')).toHaveCount(0);
			await expect(weekly.locator('button, [role=button]')).toHaveCount(0);

			const monday = weekly.getByRole('row', { name: /^จันทร์/ });
			const tuesday = weekly.getByRole('row', { name: /^อังคาร/ });
			await expect(tuesday.getByText('พักกลางวัน\nรับประทานอาหาร', { exact: true })).toBeVisible();
			await expect(tuesday.getByText('กิจกรรมพัฒนาผู้เรียน', { exact: true })).toBeVisible();
			await expect(
				weekly.getByRole('row', { name: /^เสาร์/ }).getByText('วิชาวันเสาร์')
			).toBeVisible();
			await expect(
				weekly.getByRole('row', { name: /^อาทิตย์/ }).getByText('วิชาวันอาทิตย์')
			).toBeVisible();
			await expect(weekly.getByText(longGroup, { exact: true })).toHaveCount(0);

			const stacked = monday.getByRole('cell').first().locator('[data-timetable-block]');
			await expect(stacked).toHaveCount(2);
			await expect(stacked.locator('[data-timetable-class]')).toHaveText(['ม.4/1', 'ม.4/1']);
			for (const label of await stacked
				.locator('[data-timetable-class], [data-timetable-room]')
				.all())
				await expectFullText(label);
			for (const content of [longSubject, longStructural])
				await expectFullText(monday.getByText(content, { exact: true }));
			const [first, second] = await Promise.all([
				stacked.nth(0).boundingBox(),
				stacked.nth(1).boundingBox()
			]);
			expect(second!.y).toBeGreaterThanOrEqual(first!.y + first!.height);

			const empty = weekly.getByRole('row', { name: /^พุธ/ });
			await expect(empty.getByRole('cell', { name: 'ไม่มีคาบสอน' })).toHaveCount(9);
			await expect(empty.getByText('–', { exact: true }).first()).toBeVisible();
			expect((await empty.boundingBox())!.height).toBeLessThanOrEqual(40);

			const tones = await weekly.locator('[data-timetable-block]').evaluateAll((cards) =>
				cards.map((card) => ({
					background: getComputedStyle(card).backgroundColor,
					border: getComputedStyle(card).borderTopColor
				}))
			);
			expect(new Set(tones.map((tone) => JSON.stringify(tone))).size).toBe(1);
			await expectNoOuterOverflow(page);

			const firstDay = weekly.getByRole('rowheader').first();
			const startDayX = (await firstDay.boundingBox())!.x;
			const secondPeriod = (await weekly
				.getByRole('columnheader', { name: /คาบ 2/ })
				.boundingBox())!;
			const scrollBox = (await scroll.boundingBox())!;
			expect(secondPeriod.x + secondPeriod.width).toBeLessThanOrEqual(
				scrollBox.x + scrollBox.width
			);
			const overflowX = await scroll.evaluate((node) => node.scrollWidth - node.clientWidth);
			await page.getByRole('button', { name: 'ดาวน์โหลด PDF' }).focus();
			await page.keyboard.press('Tab');
			await expect(scroll).toBeFocused();
			await page.screenshot({
				path: testInfo.outputPath(`timetable-week-${width}-${theme}.png`),
				fullPage: true,
				animations: 'disabled'
			});

			if (width < 1533) {
				expect(overflowX).toBeGreaterThan(0);
				await swipePane(page, scroll, true);
				await expect.poll(() => scroll.evaluate((node) => node.scrollLeft)).toBeGreaterThan(0);
				await scroll.hover();
				await page.mouse.wheel(10000, 0);
				await expect
					.poll(() =>
						scroll.evaluate((node) => node.scrollWidth - node.clientWidth - node.scrollLeft)
					)
					.toBeLessThanOrEqual(1);
				await page.mouse.wheel(1000, 0);
				await expectNoOuterOverflow(page);
				expect((await firstDay.boundingBox())!.x).toBeCloseTo(startDayX, 0);

				await page.mouse.wheel(-10000, 0);
				await expect.poll(() => scroll.evaluate((node) => node.scrollLeft)).toBe(0);
				await page.mouse.wheel(-1000, 0);
				await expectNoOuterOverflow(page);
				await page.keyboard.press('ArrowRight');
				await expect.poll(() => scroll.evaluate((node) => node.scrollLeft)).toBeGreaterThan(0);
				await page.keyboard.press('ArrowLeft');
				await expect.poll(() => scroll.evaluate((node) => node.scrollLeft)).toBe(0);
			} else {
				expect(overflowX).toBe(0);
			}

			expect(reads()).toBe(1);
		});
	}
}

test('synchronized activity labels show only current teacher classes and rooms', async ({
	page
}) => {
	const assigned = makeTimetableBlock('assigned', timetableIds.period1, {
		instructorIds: [staffId]
	}).groups[0];
	const other = {
		...assigned,
		id: 'other-target',
		name: 'กิจกรรม · ม.6/9 กลุ่มของครูอื่น',
		homeroomNames: ['ม.6/9'],
		instructors: [{ ...assigned.instructors[0], teacherId: timetableIds.teacherB }],
		roomName: 'ห้องของครูอื่น'
	};
	const shared = {
		...makeSynchronizedTimetableBlock('shared-groups', timetableIds.period1, [staffId]),
		groups: [{ ...assigned, name: longGroup, homeroomNames: ['ม.4/1'] }, other],
		offeringName: 'กิจกรรมพร้อมกันของทุกชั้น'
	};
	await mockStaffTimetableApis(page, [
		shared,
		{
			...shared,
			id: 'reserved',
			bellSchedulePeriodId: timetableIds.period2,
			periodName: 'คาบ 2',
			groups: [other],
			offeringName: 'กิจกรรมจองช่วงเวลา'
		}
	]);
	await page.goto('/staff/timetable');
	const weekly = page.getByTestId('own-timetable-week');
	const assignedCard = weekly.locator('[data-timetable-block="shared-groups"]');
	await expect(assignedCard.locator('[data-timetable-class]')).toHaveText('ม.4/1');
	await expect(assignedCard.locator('[data-timetable-room]')).toHaveText('ห้องคณิตศาสตร์');
	await expect(weekly.getByText(longGroup, { exact: true })).toHaveCount(0);
	await expect(weekly.getByText('ม.6/9', { exact: true })).toHaveCount(0);
	await expect(weekly.getByText('ห้องของครูอื่น', { exact: true })).toHaveCount(0);
	await expect(
		weekly.locator(
			'[data-timetable-block="reserved"] [data-timetable-class], [data-timetable-block="reserved"] [data-timetable-room]'
		)
	).toHaveCount(0);
	await expect(weekly.getByText('กิจกรรมจองช่วงเวลา', { exact: true })).toBeVisible();
});

test('compact landscape keeps period headers visible while the weekly grid scrolls vertically', async ({
	page
}) => {
	await page.setViewportSize({ width: 614, height: 400 });
	await mockStaffTimetableApis(page, workspaceEntries(), [
		'MON',
		'TUE',
		'WED',
		'THU',
		'FRI',
		'SAT',
		'SUN'
	]);
	await page.goto('/staff/timetable');
	const scroll = page.getByTestId('own-timetable-week-scroll');
	const period = scroll.getByRole('columnheader', { name: /คาบ 1/ });
	await expect(scroll).toBeVisible();
	expect((await scroll.boundingBox())!.height).toBeGreaterThan(40);
	const headerY = (await period.boundingBox())!.y;
	await expectNoOuterOverflow(page);

	await swipePane(page, scroll);
	await expect.poll(() => scroll.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
	await scroll.hover();
	await page.mouse.wheel(0, 10000);
	await expect
		.poll(() => scroll.evaluate((node) => node.scrollHeight - node.clientHeight - node.scrollTop))
		.toBeLessThanOrEqual(1);
	expect((await period.boundingBox())!.y).toBeCloseTo(headerY, 0);
	await page.mouse.wheel(0, 1000);
	await expectNoOuterOverflow(page);

	await page.mouse.wheel(0, -10000);
	await expect.poll(() => scroll.evaluate((node) => node.scrollTop)).toBe(0);
	await page.mouse.wheel(0, -1000);
	await expectNoOuterOverflow(page);
	await scroll.focus();
	await page.keyboard.press('PageDown');
	await expect.poll(() => scroll.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
	await page.keyboard.press('PageUp');
	await expect.poll(() => scroll.evaluate((node) => node.scrollTop)).toBe(0);
	await expectNoOuterOverflow(page);
});

test('own timetable keeps a configured Saturday visible when it has no lessons', async ({
	page
}) => {
	await mockStaffTimetableApis(
		page,
		[makeTimetableBlock(timetableIds.blockA, timetableIds.period1)],
		['MON', 'SAT']
	);
	await page.goto(
		`/staff/timetable?academicYearId=${academicYearId}&academicTermId=${academicTermId}`
	);
	await expect(page.getByTestId('own-timetable-week').locator('tbody tr > th')).toHaveText([
		'จันทร์',
		'เสาร์'
	]);
	await expect(
		page
			.getByTestId('own-timetable-week')
			.locator('tbody tr')
			.last()
			.locator('[data-timetable-block]')
	).toHaveCount(0);
});
