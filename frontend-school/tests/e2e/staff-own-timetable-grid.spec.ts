import { expect, test, type Page, type Route } from '@playwright/test';

import {
	makeStructuralTimetableBlock,
	makeSynchronizedTimetableBlock,
	makeTimetableBlock,
	timetableIds
} from './timetable-test-harness';

test.use({ serviceWorkers: 'block', hasTouch: true });

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

async function mockStaffTimetableApis(page: Page, items: unknown[]) {
	let timetableReads = 0;
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			if (url.pathname === '/api/auth/me') {
				await fulfillJson(route, {
					id: '10000000-0000-4000-8000-000000000001',
					username: 'teacher1',
					firstName: 'สายใจ',
					displayName: 'สายใจ วิทยา',
					lastName: 'วิทยา',
					userType: 'staff',
					status: 'ACTIVE',
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
							id: academicYearId,
							name: 'ปีการศึกษา 2569',
							year: 2569,
							startDate: '2026-05-01',
							endDate: '2027-03-31',
							status: 'active'
						}
					],
					terms: [
						{
							id: academicTermId,
							academicYearId,
							name: 'ภาคเรียนที่ 1',
							code: '1',
							startDate: '2026-05-01',
							endDate: '2026-10-31',
							sequence: 1,
							termType: 'regular',
							status: 'active',
							includedInYearResult: true,
							blocksYearClosure: true
						}
					],
					activeAcademicYearId: academicYearId,
					activeAcademicTermId: academicTermId
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

test('shows compact multiline shared cards without codes teachers or shared labels', async ({
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
	).toEqual({ whiteSpace: 'pre-line', lineClamp: '2' });

	const synchronizedTitleElement = week.getByText(synchronizedTitle, { exact: true });
	await expect(synchronizedTitleElement).toBeVisible();
	expect(
		await synchronizedTitleElement.evaluate((element) => ({
			whiteSpace: getComputedStyle(element).whiteSpace,
			lineClamp: getComputedStyle(element).webkitLineClamp
		}))
	).toEqual({ whiteSpace: 'pre-line', lineClamp: '2' });

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
const longGroup = 'กลุ่มการเรียนรู้บูรณาการห้องมัธยมศึกษาปีที่หนึ่งกลุ่มพิเศษ';

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
	entries[0].groups = entries[0].groups.map((group) => ({ ...group, name: longGroup }));
	return [
		...entries,
		{ ...entries[0], id: 'collision', offeringCode: 'คาบร่วม', offeringName: 'คาบที่จัดพร้อมกัน' },
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

for (const width of [390, 1029]) {
	for (const theme of ['light', 'dark']) {
		test(`reads daily and weekly schedules at ${width}px in ${theme}`, async ({ page }) => {
			await page.setViewportSize({ width, height: 888 });
			await page.addInitScript(
				(dark) => document.documentElement.classList.toggle('dark', dark),
				theme === 'dark'
			);
			const reads = await mockStaffTimetableApis(page, workspaceEntries());
			await page.goto('/staff/timetable');
			await expect(page.getByTestId('personal-timetable-ready')).toBeVisible();
			await page.evaluate(
				(dark) => document.documentElement.classList.toggle('dark', dark),
				theme === 'dark'
			);
			const daily = page.getByTestId('own-timetable-day');
			const weekly = page.getByTestId('own-timetable-week');
			const buttons = page
				.getByRole('radiogroup', { name: 'มุมมองตารางสอน' })
				.filter({ visible: true });
			await expect(width < 768 ? daily : weekly).toBeVisible();
			await expect(width < 768 ? weekly : daily).toBeHidden();
			await buttons.getByRole('radio', { name: 'รายวัน', exact: true }).click();
			await expect(daily.getByText(longSubject, { exact: true })).toBeVisible();
			await expect(daily.getByText(longGroup, { exact: true })).toHaveCount(2);
			await expect(daily.getByText('คาบที่จัดพร้อมกัน', { exact: true })).toBeVisible();
			await expect(daily.locator('ol > li').first().locator('[data-timetable-block]')).toHaveCount(
				2
			);
			for (const text of [longSubject, longGroup]) {
				const geometry = await daily
					.getByText(text, { exact: true })
					.first()
					.evaluate((element) => ({
						height: element.clientHeight,
						fullHeight: element.scrollHeight,
						width: element.clientWidth,
						fullWidth: element.scrollWidth
					}));
				expect(geometry.fullHeight).toBeLessThanOrEqual(geometry.height + 1);
				expect(geometry.fullWidth).toBeLessThanOrEqual(geometry.width + 1);
			}
			await page.screenshot({
				path: `/workspace/artifacts/staff-schedules/timetable-day-${width}-${theme}.png`,
				fullPage: true
			});
			const days = daily.getByRole('radiogroup', { name: 'วันในตารางสอน' });
			await days.getByRole('radio', { name: 'วันอังคาร', exact: true }).tap();
			await expect(daily.getByRole('heading', { name: 'วันอังคาร', exact: true })).toBeVisible();
			await expect(daily.getByText('ไม่มีคาบสอน', { exact: true }).first()).toBeVisible();
			await expect(daily.getByText('พักกลางวัน\nรับประทานอาหาร', { exact: true })).toBeVisible();
			await expect(daily.getByText('กิจกรรมพัฒนาผู้เรียน', { exact: true })).toBeVisible();
			await days.getByRole('radio', { name: 'วันอังคาร', exact: true }).tap();
			await expect(days.getByRole('radio', { name: 'วันอังคาร', exact: true })).toHaveAttribute(
				'data-state',
				'on'
			);
			await days.getByRole('radio', { name: 'วันเสาร์', exact: true }).focus();
			await page.keyboard.press('Enter');
			await expect(daily.getByText('วิชาวันเสาร์', { exact: true })).toBeVisible();
			await days.getByRole('radio', { name: 'วันอาทิตย์', exact: true }).click();
			await expect(daily.getByText('วิชาวันอาทิตย์', { exact: true })).toBeVisible();

			await buttons.getByRole('radio', { name: 'รายสัปดาห์', exact: true }).click();
			await expect(weekly).toBeVisible();
			const scroll = page.getByTestId('own-timetable-week-scroll');
			const size = await scroll.evaluate((element) => ({
				width: element.clientWidth,
				fullWidth: element.scrollWidth
			}));
			expect(size.fullWidth).toBeGreaterThan(size.width);
			await expect(buttons.getByRole('radio', { name: 'รายสัปดาห์', exact: true })).toBeFocused();
			await page.keyboard.press('Enter');
			await expect(buttons.getByRole('radio', { name: 'รายสัปดาห์', exact: true })).toHaveAttribute(
				'aria-checked',
				'true'
			);
			const firstCard = weekly.locator('[data-timetable-block="course-0"]');
			await page.keyboard.press('Tab');
			await expect(firstCard).toBeFocused();
			await page.keyboard.press('ArrowRight');
			await expect.poll(() => scroll.evaluate((element) => element.scrollLeft)).toBeGreaterThan(0);
			await scroll.evaluate((element) => {
				element.scrollLeft = 0;
			});
			const compactSubject = weekly.getByText(longSubject, { exact: true });
			await expect(compactSubject).toHaveCSS('-webkit-line-clamp', '2');
			const compactGeometry = await compactSubject.evaluate((element) => ({
				height: element.clientHeight,
				fullHeight: element.scrollHeight,
				lineHeight: Number.parseFloat(getComputedStyle(element).lineHeight)
			}));
			expect(compactGeometry.height).toBeGreaterThanOrEqual(
				Math.min(compactGeometry.fullHeight, compactGeometry.lineHeight * 2) - 1
			);
			const firstRow = weekly.locator('tbody > tr').first();
			await expect(firstRow.locator('td').first().locator('[data-timetable-block]')).toHaveCount(2);
			expect((await firstRow.boundingBox())!.height).toBeLessThanOrEqual(260);
			await page.keyboard.press('Enter');
			const details = page.getByRole('dialog', { name: 'รายละเอียดคาบสอน' });
			await expect(details).toBeVisible();
			await expect(details.getByText(longSubject, { exact: true })).toBeVisible();
			await expect(details.getByText(longGroup, { exact: true })).toBeVisible();
			await expect(details.getByText('MATH-1', { exact: true })).toBeVisible();
			await expect(details.getByText(/วันจันทร์ · คาบ 1/)).toBeVisible();
			for (const text of [longSubject, longGroup]) {
				const geometry = await details.getByText(text, { exact: true }).evaluate((element) => ({
					height: element.clientHeight,
					fullHeight: element.scrollHeight,
					width: element.clientWidth,
					fullWidth: element.scrollWidth
				}));
				expect(geometry.fullHeight).toBeLessThanOrEqual(geometry.height + 1);
				expect(geometry.fullWidth).toBeLessThanOrEqual(geometry.width + 1);
			}
			await page.keyboard.press('Escape');
			await expect(details).toBeHidden();
			await expect(firstCard).toBeFocused();
			await firstCard.tap();
			await expect(details.getByText(longSubject, { exact: true })).toBeVisible();
			await details.getByRole('button', { name: 'ปิดรายละเอียดคาบสอน' }).tap();
			await expect(details).toBeHidden();
			await expect(firstCard).toBeFocused();
			const collision = weekly.locator('[data-timetable-block="collision"]');
			await collision.click();
			await expect(details.getByText('คาบที่จัดพร้อมกัน', { exact: true })).toBeVisible();
			await page.keyboard.press('Escape');
			await expect(collision).toBeFocused();
			await expect(details).toHaveCount(0);
			await expect(weekly.getByText('คาบที่จัดพร้อมกัน', { exact: true })).toBeVisible();
			await scroll.evaluate((element) => element.scrollTo({ left: 0, behavior: 'instant' }));
			await page.screenshot({
				path: `/workspace/artifacts/staff-schedules/timetable-week-${width}-${theme}.png`,
				fullPage: true
			});
			await buttons.getByRole('radio', { name: 'รายวัน', exact: true }).click();
			await expect(daily.getByRole('heading', { name: 'วันอาทิตย์', exact: true })).toBeVisible();
			expect(
				await page.evaluate(
					() => document.documentElement.scrollWidth - document.documentElement.clientWidth
				)
			).toBeLessThanOrEqual(1);
			expect(reads()).toBe(1);
		});
	}
}
