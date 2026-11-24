import { revalidatePage } from './fixtures/page-revalidation';
import { expect, test, type Locator, type Page, type Request } from '@playwright/test';
import {
	id,
	year,
	nextYear,
	templateId,
	timetableOption,
	path,
	mock,
	navigate
} from './fixtures/supervision-route-data';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.clock.setFixedTime(new Date('2026-09-30T09:00:00Z'));
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
const assigned = ['supervision.read.assigned', 'supervision.evaluate.assigned'];
const own = ['supervision.read.own', 'supervision.request.own'];
const observationPath = '/api/supervision/observations';

for (const hold of ['cycles', 'summaries'] as const) {
	test(`requests list renders independently of delayed ${hold}`, async ({ page }) => {
		const api = await mock(page, { hold });
		await page.goto(path('requests'));
		await expect(
			page.getByTestId('supervision-observations').getByText('ครู ในคิว', { exact: true }).first()
		).toBeVisible();
		await expect(page.getByTestId('supervision-references').getByRole('status')).toBeVisible();
		expect(api.count(`/api/supervision/observations/${id(30)}/evaluator-availability`)).toBe(0);
		expect(api.count(`/api/supervision/templates/${templateId}`)).toBe(0);
		api.release();
		await expect(page.getByTestId('supervision-references').getByRole('status')).toHaveCount(0);
	});
}

test('requests first list is a skeleton, with no false empty state', async ({ page }) => {
	const api = await mock(page, { hold: 'observations' });
	await page.goto(path('requests'));
	await expect(page.getByTestId('supervision-observations').getByRole('status')).toBeVisible();
	await expect(page.getByText('ไม่มีคำขอจองที่รออนุมัติ')).toHaveCount(0);
	api.release();
	await expect(page.getByText('ครู ในคิว', { exact: true }).first()).toBeVisible();
});

test('queue retry rereads only its failed observation list', async ({ page }) => {
	const api = await mock(page, { fail: 'observations' });
	await page.goto(path('requests'));
	const region = page.getByTestId('supervision-observations');
	await expect(region.getByText('region ไม่พร้อม')).toBeVisible();
	await region.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(region.getByText('ครู ในคิว', { exact: true }).first()).toBeVisible();
	expect(api.count(observationPath)).toBe(2);
	expect(api.count('/api/supervision/cycles')).toBe(1);
	expect(api.count('/api/supervision/templates/summaries')).toBe(1);
});

test('queue refresh retains usable rows through an error', async ({ page }) => {
	const api = await mock(page, { fail: 'observations', failAt: 2 });
	await page.goto(path('requests'));
	const region = page.getByTestId('supervision-observations');
	await expect(region.getByText('ครู ในคิว', { exact: true }).first()).toBeVisible();
	await revalidatePage(page);
	await expect(region.getByText('region ไม่พร้อม')).toBeVisible();
	await expect(region.getByText('ครู ในคิว', { exact: true }).first()).toBeVisible();
	await region.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(region.getByText('region ไม่พร้อม')).toHaveCount(0);
	expect(api.count(observationPath)).toBe(3);
	expect(api.count('/api/supervision/cycles')).toBe(2);
});

test('a late old-year list cannot overwrite the next year', async ({ page }) => {
	const api = await mock(page, { hold: 'observations' });
	await page.goto(path('requests'));
	await expect(page.getByTestId('supervision-observations').getByRole('status')).toBeVisible();
	await navigate(page, path('requests', nextYear));
	await expect(page.getByText('ครู ปีใหม่', { exact: true }).first()).toBeVisible();
	api.release();
	await expect(page.getByText('ครู ในคิว', { exact: true })).toHaveCount(0);
});

test('returning a request supersedes a pending stale refresh', async ({ page }) => {
	const api = await mock(page, { hold: 'observations', holdAt: 2 });
	await page.goto(path('requests'));
	await expect(page.getByRole('button', { name: 'ส่งกลับคำขอ', exact: true })).toBeVisible();
	await revalidatePage(page);
	await expect(page.getByTestId('supervision-observations')).toHaveAttribute('aria-busy', 'true');
	await page.getByRole('button', { name: 'ส่งกลับคำขอ', exact: true }).click();
	await expect(page.getByText('ไม่มีคำขอจองที่รออนุมัติ')).toBeVisible();
	api.release();
	await expect(page.getByRole('button', { name: 'ส่งกลับคำขอ', exact: true })).toHaveCount(0);
	expect(api.count(observationPath)).toBe(2);
});

test('evaluator picker loads on open and retries only its options', async ({ page }) => {
	const api = await mock(page, { fail: 'evaluator' });
	await page.goto(path('requests'));
	await page.getByRole('combobox', { name: /เลือกผู้ประเมิน/ }).click();
	await expect(page.getByText('region ไม่พร้อม')).toBeVisible();
	await page.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByText('ผู้ประเมินที่ว่าง', { exact: true })).toBeVisible();
	expect(api.count(observationPath)).toBe(1);
	expect(api.count(`/api/supervision/observations/${id(30)}/evaluator-availability`)).toBe(2);
});

test('evaluation rubric is lazy and a typed submit patches only the observation', async ({
	page
}) => {
	const api = await mock(page, {
		permissions: assigned,
		observationStatus: 'planned',
		fail: 'detail'
	});
	await page.goto(path('evaluate'));
	await expect(page.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true })).toBeVisible();
	expect(api.count(`/api/supervision/templates/${templateId}`)).toBe(0);
	await page.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByText('region ไม่พร้อม')).toBeVisible();
	await dialog.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(dialog.getByText('หัวข้อ rubric ที่โหลดเมื่อเปิด')).toBeVisible();
	await dialog.getByRole('button', { name: '3', exact: true }).click();
	await dialog.getByRole('button', { name: 'ส่งผลประเมิน', exact: true }).click();
	await expect(page.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true })).toHaveCount(0);
	expect(api.count(observationPath)).toBe(1);
	expect(api.count('/api/supervision/templates/summaries')).toBe(1);
});

test('closing evaluation supersedes its pending rubric', async ({ page }) => {
	const api = await mock(page, {
		permissions: assigned,
		hold: 'detail',
		observationStatus: 'planned'
	});
	await page.goto(path('evaluate'));
	await page.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true }).click();
	await expect(page.getByRole('dialog').getByRole('status')).toBeVisible();
	await page.keyboard.press('Escape');
	api.release();
	await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('write-only evaluation permission does not grant observation reads', async ({ page }) => {
	const api = await mock(page, { permissions: ['supervision.evaluate.assigned'] });
	await page.goto(path('evaluate'));
	await expect(page.getByText('ยังไม่มีสิทธิ์ดูรายการนิเทศ')).toBeVisible();
	await expect(page.getByRole('tablist', { name: 'มุมมองรายการประเมิน' })).toHaveCount(0);
	expect(api.count(observationPath)).toBe(0);
	expect(api.count('/api/supervision/cycles')).toBe(0);
	expect(api.count('/api/supervision/templates/summaries')).toBe(0);
});

test('an empty successful evaluation read keeps both views available', async ({ page }) => {
	const api = await mock(page, { permissions: assigned });
	api.setObservationScope('hidden');
	await page.goto(path('evaluate'));
	await expect(page.getByText('ยังไม่มีรายการที่ต้องประเมิน')).toBeVisible();
	await page.getByRole('tab', { name: 'ปฏิทิน', exact: true }).click();
	await expect(page.getByText('ไม่มีรายการนิเทศที่ได้รับมอบหมายในวันนี้')).toBeVisible();
	expect(api.count(observationPath)).toBe(1);
});

async function submitFromCalendar(page: Page) {
	await page.goto(path('evaluate'));
	await page.getByRole('tab', { name: 'ปฏิทิน', exact: true }).click();
	const calendar = page.getByRole('tabpanel', { name: 'ปฏิทิน', exact: true });
	const day = calendar.locator('[data-calendar-date="2026-10-01"]');
	await day.click();
	await expect(day).toHaveAttribute('aria-pressed', 'true');
	await calendar.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByText('หัวข้อ rubric ที่โหลดเมื่อเปิด')).toBeVisible();
	await dialog.getByRole('button', { name: '3', exact: true }).click();
	const submit = dialog.getByRole('button', { name: 'ส่งผลประเมิน', exact: true });
	await expect(submit).toBeInViewport();
	const submitPath = `${observationPath}/${id(30)}/evaluations/me/submit`;
	const pending = page.waitForRequest((request) => new URL(request.url()).pathname === submitPath);
	await submit.click();
	return { calendar, day, dialog, request: await pending };
}

async function revalidateWhileEvaluating(dialog: Locator) {
	// Dispatch same-URL navigation from inside the modal while its submit is pending.
	await dialog.evaluate((content) => {
		const link = document.createElement('a');
		link.href = window.location.href;
		link.dataset.sveltekitPreloadData = 'off';
		content.append(link);
		link.click();
		link.remove();
	});
}

async function finishSuccessfulRequest(page: Page, request: Request) {
	const response = await request.response();
	expect(response?.ok()).toBe(true);
	await response?.finished();
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);
}

test('a denied evaluation refresh closes the form and discards an in-flight submit result', async ({
	page
}) => {
	const api = await mock(page, {
		permissions: assigned,
		observationStatus: 'planned',
		hold: 'patch',
		fail: 'observations',
		failAt: 2,
		failStatus: 403
	});
	const { calendar, day, dialog, request } = await submitFromCalendar(page);
	await revalidateWhileEvaluating(dialog);
	await expect(page.getByText('ยังไม่มีสิทธิ์ดูรายการนิเทศ')).toBeVisible();
	await expect(dialog).toHaveCount(0);
	await expect(page.getByRole('tablist', { name: 'มุมมองรายการประเมิน' })).toHaveCount(0);
	await expect(page.getByText('ครู ในคิว', { exact: true })).toHaveCount(0);

	api.setObservationScope('hidden');
	await revalidatePage(page);
	await expect(calendar.getByText('ไม่มีรายการนิเทศที่ได้รับมอบหมายในวันนี้')).toBeVisible();
	api.release();
	await finishSuccessfulRequest(page, request);
	await day.click();
	await expect(page.getByText('ส่งผลประเมินแล้ว', { exact: true })).toHaveCount(0);
	await expect(calendar.getByText('ไม่มีรายการนิเทศที่ได้รับมอบหมายในวันนี้')).toBeVisible();
	expect(api.count(observationPath)).toBe(3);
});

test('an evaluation submit overlapping a refresh still applies a fresh authorization decision', async ({
	page
}) => {
	const api = await mock(page, {
		permissions: assigned,
		observationStatus: 'planned',
		hold: 'patch'
	});
	const { calendar, dialog, request } = await submitFromCalendar(page);
	const releaseOlderRead = Promise.withResolvers<void>();
	const releaseFreshRead = Promise.withResolvers<void>();
	const staleObservation = api.observation();
	let refreshes = 0;
	await page.route(
		(url) => url.pathname === observationPath,
		async (route) => {
			if (route.request().method() !== 'GET') return route.fallback();
			if (++refreshes === 1) {
				await releaseOlderRead.promise;
				return route.fulfill({ json: { success: true, data: { items: [staleObservation] } } });
			}
			await releaseFreshRead.promise;
			return route.fulfill({ status: 403, json: { success: false, error: 'denied' } });
		}
	);
	const pendingRead = page.waitForRequest(
		(read) => read.method() === 'GET' && new URL(read.url()).pathname === observationPath
	);
	await revalidateWhileEvaluating(dialog);
	const olderRead = await pendingRead;
	await expect(page.getByTestId('supervision-observations')).toHaveAttribute('aria-busy', 'true');
	api.release();
	await finishSuccessfulRequest(page, request);
	await expect.poll(() => refreshes).toBe(2);
	await expect(dialog).toHaveCount(0);
	await expect(calendar.getByText('ส่งผลแล้ว', { exact: true })).toBeVisible();

	releaseFreshRead.resolve();
	const denied = page.getByText('ยังไม่มีสิทธิ์ดูรายการนิเทศ');
	await expect(denied).toBeVisible();
	releaseOlderRead.resolve();
	await finishSuccessfulRequest(page, olderRead);
	await expect(denied).toBeVisible();
	await expect(page.getByRole('tablist', { name: 'มุมมองรายการประเมิน' })).toHaveCount(0);
	await expect(page.getByText('ครู ในคิว', { exact: true })).toHaveCount(0);
	expect(api.count(observationPath)).toBe(1);
	expect(refreshes).toBe(2);
});

for (const closeBeforeRefresh of [false, true]) {
	test(`a successful evaluation refresh discards a withdrawn assignment with the form ${closeBeforeRefresh ? 'closed' : 'open'}`, async ({
		page
	}) => {
		const api = await mock(page, {
			permissions: assigned,
			observationStatus: 'planned',
			hold: 'patch'
		});
		const { calendar, day, dialog, request } = await submitFromCalendar(page);
		api.setObservationScope('hidden');
		if (closeBeforeRefresh) {
			await page.keyboard.press('Escape');
			await expect(dialog).toHaveCount(0);
			await revalidatePage(page);
		} else {
			await revalidateWhileEvaluating(dialog);
		}
		await expect(dialog).toHaveCount(0);
		await expect(day).toHaveAttribute('aria-pressed', 'true');
		const emptyAgenda = calendar.getByText('ไม่มีรายการนิเทศที่ได้รับมอบหมายในวันนี้');
		await expect(emptyAgenda).toBeVisible();
		api.release();
		await finishSuccessfulRequest(page, request);
		await expect(page.getByText('ส่งผลประเมินแล้ว', { exact: true })).toHaveCount(0);
		await expect(emptyAgenda).toBeVisible();
		expect(api.count(observationPath)).toBe(2);
	});
}

test('mine loads its list first and keeps booking timetable behind opening the workflow', async ({
	page
}) => {
	const api = await mock(page, { permissions: own, term: true, fail: 'timetable' });
	await page.goto(path('').replace('/supervision/?', '/supervision?'));
	await expect(
		page
			.getByTestId('supervision-observations')
			.getByRole('heading', { name: 'วิชาทดสอบ', exact: true })
	).toBeVisible();
	expect(api.count('/api/supervision/booking-options')).toBe(0);
	await page.getByRole('button', { name: 'จองคาบนิเทศ', exact: true }).click();
	await expect(page.getByText('region ไม่พร้อม')).toBeVisible();
	await page.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByText('ไม่พบคาบสอนในสัปดาห์นี้')).toBeVisible();
	expect(api.count('/api/supervision/booking-options')).toBe(2);
	expect(api.count(observationPath)).toBe(1);
});

test.describe('supervision at 360 pixels', () => {
	test.use({ viewport: { width: 360, height: 800 }, hasTouch: true });

	test('booking shows two complete lesson columns with the date pinned while scrolling', async ({
		page
	}) => {
		const longSubject = 'คณิตศาสตร์พื้นฐานและการแก้ปัญหาในชีวิตประจำวันสำหรับนักเรียนมัธยมศึกษา';
		const adjacentSubject = 'ภาษาไทยสำหรับนักเรียนมัธยมต้น';
		const adjacentRoom = 'ห้องปฏิบัติการวิทยาศาสตร์';
		const stackedSlot = {
			observedDate: '2026-09-30',
			startTime: '08:00:00',
			endTime: '09:00:00',
			periodName: 'คาบ 1'
		};
		await mock(page, {
			permissions: own,
			term: true,
			bookingItems: [
				timetableOption(0),
				timetableOption(1, {
					subjectName: adjacentSubject,
					roomName: adjacentRoom,
					bookingAvailability: { pendingCount: 2, confirmedCount: 1, blocked: false }
				}),
				timetableOption(2),
				timetableOption(3),
				timetableOption(4, { ...stackedSlot, subjectName: longSubject, homeroomName: 'ม.2/1' }),
				timetableOption(5, { ...stackedSlot, homeroomName: 'ม.2/2' })
			]
		});
		await page.goto(path('').replace('/supervision/?', '/supervision?'));
		await page.getByRole('button', { name: 'จองคาบนิเทศ', exact: true }).tap();
		const grid = page.getByTestId('supervision-booking-grid');
		const rowDates = grid.getByRole('rowheader').locator('time');
		const expectUniformDayRows = async () => {
			const heights = await grid
				.getByRole('rowheader')
				.evaluateAll((headers) =>
					headers.map((header) => header.closest('tr')!.getBoundingClientRect().height)
				);
			expect(heights).toHaveLength(5);
			expect(Math.max(...heights) - Math.min(...heights)).toBeLessThan(1);
		};
		await expect(rowDates).toHaveCount(5);
		expect(
			await rowDates.evaluateAll((dates) => dates.map((date) => date.getAttribute('datetime')))
		).toEqual(['2026-09-28', '2026-09-29', '2026-09-30', '2026-10-01', '2026-10-02']);
		await expectUniformDayRows();
		const stackedCell = grid.getByRole('cell').filter({ hasText: longSubject });
		await expect(stackedCell.getByRole('button')).toHaveCount(2);
		await expect(stackedCell.getByText(longSubject, { exact: true })).toBeVisible();
		expect(await stackedCell.evaluate((cell) => cell.scrollHeight <= cell.clientHeight)).toBe(true);
		const thursday = grid.locator('time[datetime="2026-10-01"]');
		await expect(thursday.locator('span')).toHaveText(['พฤหัส', '1 ต.ค.']);
		const weekday = await thursday.locator('span').first().boundingBox();
		const date = await thursday.locator('span').last().boundingBox();
		expect(date!.y).toBeGreaterThanOrEqual(weekday!.y + weekday!.height);

		const scrollport = grid.locator('[data-slot="table-container"]');
		const initial = await scrollport.boundingBox();
		const secondPeriod = await grid.getByRole('columnheader', { name: /^คาบ 2 / }).boundingBox();
		expect(secondPeriod!.x + secondPeriod!.width).toBeLessThanOrEqual(initial!.x + initial!.width);
		expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(360);
		const first = grid.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ });
		const second = grid.getByRole('button', { name: /^เลือก คาบ 2 ม.1\/1/ });
		const third = grid.getByRole('button', { name: /^เลือก คาบ 3 ม.1\/1/ });
		const fourth = grid.getByRole('button', { name: /^เลือก คาบ 4 ม.1\/1/ });
		const otherDay = grid.getByRole('button', { name: /^เลือก คาบ 1 ม.2\/1/ });
		const subjectLabel = second.getByText(adjacentSubject, { exact: true });
		const statusLabel = second.getByText('รออนุมัติ 2 · นัดหมาย 1 · จองขนานได้', { exact: true });
		const roomLabel = second.getByText(adjacentRoom, { exact: true });
		const cardSizes = () =>
			Promise.all(
				[second, subjectLabel, roomLabel, statusLabel].map(async (part) => {
					const { width, height } = (await part.boundingBox())!;
					return { width, height };
				})
			);
		await expect(grid.getByRole('button')).toHaveCount(6);
		await expect(third).toBeEnabled();
		await expect(otherDay).toBeEnabled();
		const sizesBefore = await cardSizes();
		await expect(first).toHaveAccessibleDescription(/วิชาปัจจุบัน 1.*ว่าง/);
		const unselectedFill = await first.evaluate(
			(button) => getComputedStyle(button).backgroundColor
		);
		await first.focus();
		await first.press('Enter');
		await expect(first).toHaveAttribute('aria-pressed', 'true');
		await expect(first).toBeEnabled();
		await expect
			.poll(() => first.evaluate((button) => getComputedStyle(button).backgroundColor))
			.not.toBe(unselectedFill);
		await expect(grid.getByText('เลือกแล้ว', { exact: true })).toHaveCount(0);
		await expect(grid.getByRole('button')).toHaveCount(6);
		await expect(third).toHaveJSProperty('disabled', true);
		await expect(otherDay).toHaveJSProperty('disabled', true);
		expect(await cardSizes()).toEqual(sizesBefore);
		await expectUniformDayRows();
		await page.keyboard.press('Tab');
		await expect(second).toBeFocused();
		await second.tap();
		const selection = page.getByTestId('supervision-booking-selection');
		await expect(selection).toContainText('2 คาบต่อเนื่อง');
		await expect(first).toHaveAttribute('aria-pressed', 'true');
		await expect(second).toHaveAttribute('aria-pressed', 'true');
		await expect(third).toBeEnabled();
		await expect(fourth).toHaveJSProperty('disabled', true);
		expect(await cardSizes()).toEqual(sizesBefore);
		await expectUniformDayRows();
		await second.tap();
		await expect(first).toHaveAttribute('aria-pressed', 'true');
		await expect(second).toHaveAttribute('aria-pressed', 'false');
		await expect(third).toBeDisabled();
		await expect(selection).toContainText('1 คาบต่อเนื่อง · 08:00–09:00');
		await second.tap();
		await first.press('Space');
		await expect(first).toHaveAttribute('aria-pressed', 'false');
		await expect(second).toHaveAttribute('aria-pressed', 'true');
		await expect(third).toBeEnabled();
		await expect(selection).toContainText('1 คาบต่อเนื่อง · 09:00–10:00');
		await second.tap();
		await expect(second).toHaveAttribute('aria-pressed', 'false');
		await expect(fourth).toBeEnabled();
		await expect(otherDay).toBeEnabled();
		await expect(selection).toContainText('ยังไม่ได้เลือกคาบ');
		await expect(selection.getByRole('button', { name: 'ส่งคำขอจอง', exact: true })).toBeDisabled();
		expect(await cardSizes()).toEqual(sizesBefore);
		await expectUniformDayRows();

		const pinned = grid.getByRole('columnheader', { name: 'วันที่' });
		const pinnedBefore = await pinned.boundingBox();
		const scrolled = await scrollport.evaluate((element) => {
			element.scrollLeft = element.scrollWidth;
			return element.scrollLeft;
		});
		expect(scrolled).toBeGreaterThan(0);
		const pinnedAfter = await pinned.boundingBox();
		expect(pinnedAfter?.x).toBe(pinnedBefore?.x);
	});

	test('booking uses the custom term timetable and keeps open days without lessons', async ({
		page
	}) => {
		await mock(page, {
			permissions: own,
			term: true,
			schoolDays: ['SUN', 'SAT', 'THU', 'TUE'],
			bookingItems: [
				timetableOption(0),
				timetableOption(1, { observedDate: '2026-10-03' }),
				timetableOption(2, { observedDate: '2026-10-02' })
			]
		});
		await page.goto(path('').replace('/supervision/?', '/supervision?'));
		await page.getByRole('button', { name: 'จองคาบนิเทศ', exact: true }).tap();
		const grid = page.getByTestId('supervision-booking-grid');
		const rowDates = grid.getByRole('rowheader').locator('time');
		await expect(rowDates).toHaveCount(4);
		expect(
			await rowDates.evaluateAll((dates) => dates.map((date) => date.getAttribute('datetime')))
		).toEqual(['2026-09-29', '2026-10-01', '2026-10-03', '2026-10-04']);
		const emptyTeachingDay = grid
			.getByRole('row')
			.filter({ has: page.locator('time[datetime="2026-09-29"]') });
		await expect(emptyTeachingDay).toBeVisible();
		await expect(emptyTeachingDay.getByRole('button')).toHaveCount(0);
		await expect(grid.getByRole('columnheader', { name: /^คาบ 3 / })).toHaveCount(0);
		const saturday = grid.getByRole('button', { name: /^เลือก คาบ 2 ม.1\/1/ });
		await saturday.tap();
		await expect(saturday).toHaveAttribute('aria-pressed', 'true');
	});

	test('evaluation calendar is as wide as the requests calendar and still opens the rubric', async ({
		page
	}) => {
		await mock(page, {
			permissions: ['supervision.manage.school', 'supervision.read.school', ...assigned],
			observationStatus: 'planned'
		});
		const tab = page.getByRole('tab', { name: 'ปฏิทิน', exact: true });
		const panel = page.getByRole('tabpanel', { name: 'ปฏิทิน', exact: true });
		const firstDate = panel.locator('[data-calendar-date="2026-09-27"]');
		const lastDate = panel.locator('[data-calendar-date="2026-10-03"]');
		await page.goto(path('requests'));
		await tab.tap();
		const requestsFirst = await firstDate.boundingBox();
		const requestsLast = await lastDate.boundingBox();
		await page.goto(path('evaluate'));
		await expect(
			page.getByRole('heading', { name: 'รายการที่ได้รับมอบหมายให้ประเมิน' })
		).toBeVisible();
		await tab.tap();
		const evaluationFirst = await firstDate.boundingBox();
		const evaluationLast = await lastDate.boundingBox();
		expect(evaluationFirst?.x).toBe(requestsFirst?.x);
		expect(evaluationLast!.x + evaluationLast!.width).toBe(requestsLast!.x + requestsLast!.width);
		const day = panel.locator('[data-calendar-date="2026-10-01"]');
		await day.tap();
		await expect(day).toHaveAttribute('aria-pressed', 'true');
		await panel.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true }).tap();
		await expect(
			page.getByRole('dialog').getByText('หัวข้อ rubric ที่โหลดเมื่อเปิด')
		).toBeVisible();
	});
});

test('the retired approvals URL has no approval view or observation reads', async ({ page }) => {
	const api = await mock(page);
	await page.goto(path('approvals'));
	await expect(page.getByRole('heading', { name: 'ไม่พบรายการนิเทศ', exact: true })).toBeVisible();
	await expect(page.getByRole('link', { name: /รับรอง\/อนุมัติ/ })).toHaveCount(0);
	expect(api.reads.filter((value) => value.startsWith('/api/supervision/'))).toEqual([]);
});

test('denied queue route cannot preload an otherwise readable list', async ({ page }) => {
	const api = await mock(page, { permissions: ['supervision.read.own'] });
	await page.goto(path('requests', year));
	await expect(page).toHaveURL(/\/403/);
	expect(api.count(observationPath)).toBe(0);
});
