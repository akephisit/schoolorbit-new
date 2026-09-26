import { expect, test, type Route } from '@playwright/test';

import { installTimetableMock, makeTimetableBlock, timetableIds } from './timetable-test-harness';

test.use({ serviceWorkers: 'block' });

const secondTermId = '21000000-0000-4000-8000-000000000202';

function todayUrl(termId: string = timetableIds.term): string {
	return `/staff/academic/timetable/today?academicYearId=${timetableIds.year}&academicTermId=${termId}`;
}

function personalUrl(termId: string = timetableIds.term): string {
	return `/staff/timetable?academicYearId=${timetableIds.year}&academicTermId=${termId}`;
}

function fulfill(route: Route, data: unknown): Promise<void> {
	return route.fulfill({
		status: 200,
		contentType: 'application/json',
		body: JSON.stringify({ success: true, data })
	});
}

function overview(route: Route, teacherName: string) {
	const url = new URL(route.request().url());
	return {
		academicTermId: url.searchParams.get('academicTermId'),
		date: url.searchParams.get('date'),
		dayOfWeek: 'MON',
		periods: [
			{
				id: timetableIds.period1,
				orderIndex: 1,
				name: 'คาบ 1',
				startTime: '08:30:00',
				endTime: '09:20:00'
			}
		],
		summary: {
			displayedTeacherCount: 1,
			emptyTeacherCount: 0,
			lessonCount: 1,
			teachersTeachingCount: 1,
			totalTeacherCount: 1
		},
		teachers: [
			{
				id: timetableIds.teacherA,
				displayName: teacherName,
				periods: [{ bellSchedulePeriodId: timetableIds.period1, entries: [] }]
			}
		]
	};
}

test('daily overview shows a first skeleton and starts exactly one route read', async ({
	page
}) => {
	await installTimetableMock(page);
	let release: ((route: Route) => void) | undefined;
	const pending = new Promise<Route>((resolve) => (release = resolve));
	let requests = 0;
	await page.route('**/api/academic/timetable/daily-teaching**', (route) => {
		requests += 1;
		release?.(route);
	});

	await page.goto(todayUrl());
	const route = await pending;
	await expect(page.getByText('ครูเริ่มต้น')).toHaveCount(0);
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	await fulfill(route, overview(route, 'ครูเริ่มต้น'));
	await expect(page.getByTestId('daily-teaching-ready')).toBeVisible();
	await expect(page.getByText('ครูเริ่มต้น')).toBeVisible();
	expect(requests).toBe(1);
});

test('daily date changes clear old data and late replies cannot overwrite the latest date', async ({
	page
}) => {
	await installTimetableMock(page);
	const pending: Route[] = [];
	await page.route('**/api/academic/timetable/daily-teaching**', (route) => {
		pending.push(route);
	});
	await page.goto(todayUrl());
	await expect.poll(() => pending.length).toBe(1);
	await fulfill(pending[0], overview(pending[0], 'ครูวันเริ่มต้น'));
	await expect(page.getByText('ครูวันเริ่มต้น')).toBeVisible();

	await page.getByRole('button', { name: 'วันถัดไป' }).click();
	await expect.poll(() => pending.length).toBe(2);
	await expect(page.getByText('ครูวันเริ่มต้น')).toHaveCount(0);
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	await page.getByRole('button', { name: 'วันถัดไป' }).click();
	await expect.poll(() => pending.length).toBe(3);
	await fulfill(pending[2], overview(pending[2], 'ครูวันล่าสุด'));
	await expect(page.getByText('ครูวันล่าสุด')).toBeVisible();
	await fulfill(pending[1], overview(pending[1], 'ครูวันเก่า')).catch(() => undefined);
	await expect(page.getByText('ครูวันเก่า')).toHaveCount(0);
	await page.evaluate((href) => {
		const link = document.createElement('a');
		link.href = href;
		link.id = 'test-same-term-navigation';
		link.textContent = 'โหลด route เดิมซ้ำ';
		link.style.cssText = 'position:fixed;top:160px;left:500px;z-index:9999;background:white';
		document.body.append(link);
	}, `${todayUrl()}&probe=1`);
	await page.locator('#test-same-term-navigation').click();
	await expect(page).toHaveURL(/probe=1/);
	await expect(page.getByText('ครูวันล่าสุด')).toBeVisible();
	expect(pending).toHaveLength(3);
});

test('daily refresh retains usable data and exposes a local retry after failure', async ({
	page
}) => {
	await installTimetableMock(page);
	const pending: Route[] = [];
	await page.route('**/api/academic/timetable/daily-teaching**', (route) => {
		pending.push(route);
	});
	await page.goto(todayUrl());
	await expect.poll(() => pending.length).toBe(1);
	await fulfill(pending[0], overview(pending[0], 'ครูข้อมูลเดิม'));
	const ready = page.getByTestId('daily-teaching-ready');
	await expect(ready).toBeVisible();
	await page.getByRole('button', { name: 'รีเฟรช' }).click();
	await expect.poll(() => pending.length).toBe(2);
	await expect(ready).toHaveAttribute('aria-busy', 'true');
	await expect(page.getByText('ครูข้อมูลเดิม')).toBeVisible();
	await expect(page.locator('[data-slot="skeleton"]')).toHaveCount(0);
	await pending[1].fulfill({
		status: 500,
		contentType: 'application/json',
		body: JSON.stringify({ success: false, error: 'โหลดไม่สำเร็จ' })
	});
	await expect(ready.getByRole('alert')).toBeVisible();
	await expect(page.getByText('ครูข้อมูลเดิม')).toBeVisible();
	await ready.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect.poll(() => pending.length).toBe(3);
	await fulfill(pending[2], overview(pending[2], 'ครูข้อมูลใหม่'));
	await expect(page.getByText('ครูข้อมูลใหม่')).toBeVisible();
	await expect(ready.getByRole('alert')).toHaveCount(0);
});

test('personal timetable starts once in the route and clears the old term before paint', async ({
	page
}) => {
	await installTimetableMock(page);
	await page.route('**/api/academic/context/options', (route) =>
		fulfill(route, {
			activeAcademicYearId: timetableIds.year,
			activeAcademicTermId: timetableIds.term,
			years: [
				{
					id: timetableIds.year,
					name: 'ปีการศึกษา 2569',
					year: 2569,
					status: 'active',
					startDate: '2026-05-01',
					endDate: '2027-03-31'
				}
			],
			terms: [timetableIds.term, secondTermId].map((id, index) => ({
				id,
				academicYearId: timetableIds.year,
				name: `ภาคเรียนที่ ${index + 1}`,
				code: String(index + 1),
				sequence: index + 1,
				termType: 'regular',
				status: 'active',
				startDate: '2026-05-01',
				endDate: '2027-03-31',
				includedInYearResult: true,
				blocksYearClosure: true
			}))
		})
	);
	const pending: Route[] = [];
	await page.route('**/api/me/timetable**', (route) => {
		pending.push(route);
	});

	await page.goto(personalUrl());
	await expect.poll(() => pending.length).toBe(1);
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	await fulfill(pending[0], [
		makeTimetableBlock(timetableIds.blockA, timetableIds.period1, { name: 'วิชาภาคหนึ่ง' })
	]);
	await expect(page.getByTestId('personal-timetable-ready')).toBeVisible();
	await expect(page.getByText('วิชาภาคหนึ่ง')).toBeVisible();

	await page.evaluate((href) => {
		const link = document.createElement('a');
		link.href = href;
		link.id = 'test-term-navigation';
		link.textContent = 'เปลี่ยนภาคเรียน';
		link.style.cssText = 'position:fixed;top:160px;left:500px;z-index:9999;background:white';
		document.body.append(link);
	}, personalUrl(secondTermId));
	await page.locator('#test-term-navigation').click();
	await expect.poll(() => pending.length).toBe(2);
	await expect(page.getByText('วิชาภาคหนึ่ง')).toHaveCount(0);
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	await fulfill(pending[1], [
		makeTimetableBlock(timetableIds.blockB, timetableIds.period1, { name: 'วิชาภาคสอง' })
	]);
	await expect(page.getByText('วิชาภาคสอง')).toBeVisible();
	expect(pending).toHaveLength(2);
});

test('personal timetable shows a focused error and retries only its own read', async ({ page }) => {
	await installTimetableMock(page);
	let requests = 0;
	await page.route('**/api/me/timetable**', (route) => {
		requests += 1;
		if (requests === 1) {
			void route.fulfill({
				status: 500,
				contentType: 'application/json',
				body: JSON.stringify({ success: false, error: 'โหลดไม่สำเร็จ' })
			});
		} else {
			void fulfill(route, [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)]);
		}
	});
	await page.goto(personalUrl());
	await expect(page.getByText('โหลดตารางสอนไม่สำเร็จ', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByTestId('personal-timetable-ready')).toBeVisible();
	expect(requests).toBe(2);
});
