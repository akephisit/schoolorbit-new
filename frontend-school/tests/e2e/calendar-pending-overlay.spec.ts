import { test, expect, type Page, type Route } from '@playwright/test';
import { mockCalendar, calendarPath, eventsPath } from './fixtures/calendar-route-data';
import { id } from './fixtures/staff-home-route-data';
import type { PendingCalendarRequest, CalendarEventRequest } from '../../src/lib/api/calendar';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.clock.setFixedTime(new Date('2026-10-01T02:00:00Z'));
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
const endpoint = '/api/calendar/requests/calendar';
const pending: PendingCalendarRequest[] = [
	{
		id: id(140),
		title: 'คำร้องเวลาเช้า',
		startDate: '2026-10-01',
		endDate: '2026-10-01',
		allDay: false,
		startTime: '08:00:00',
		endTime: '09:00:00'
	},
	{
		id: id(141),
		title: 'คำร้องหลายวัน',
		startDate: '2026-09-30',
		endDate: '2026-10-03',
		allDay: true,
		startTime: null,
		endTime: null
	}
];
const reply = (route: Route, records: PendingCalendarRequest[], hasMore = false) =>
	route.fulfill({
		status: 200,
		contentType: 'application/json',
		body: JSON.stringify({ success: true, data: { records, hasMore } })
	});
async function mockOverlay(
	page: Page,
	options: {
		holdFirst?: boolean;
		failFirst?: boolean;
		hasMore?: boolean;
		records?: PendingCalendarRequest[];
	} = {}
) {
	const reads: URL[] = [];
	let release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route(
		(url) => url.pathname === endpoint,
		async (route) => {
			const url = new URL(route.request().url());
			reads.push(url);
			const first = reads.length === 1;
			const november = url.searchParams.get('from')?.startsWith('2026-11');
			if (first && options.holdFirst) await held;
			if (first && options.failFirst)
				return route.fulfill({
					status: 503,
					contentType: 'application/json',
					body: JSON.stringify({ success: false, error: 'คำร้องไม่พร้อม ลองใหม่' })
				});
			return reply(
				route,
				november
					? [
							{
								...pending[0],
								title: 'คำร้องพฤศจิกายน',
								startDate: '2026-11-10',
								endDate: '2026-11-10'
							}
						]
					: (options.records ?? pending),
				options.hasMore
			);
		}
	);
	return { reads, release };
}
const toggle = (page: Page) =>
	page.getByRole('button', { name: 'แสดงคำร้องรออนุมัติ', exact: true });

for (const width of [1280, 375]) {
	test(`pending dates remain distinct from confirmed activities, with an optional overlay at ${width}px`, async ({
		page
	}, testInfo) => {
		await page.setViewportSize({ width, height: 900 });
		const api = await mockCalendar(page);
		const overlay = await mockOverlay(page);
		await page.goto(calendarPath());
		await expect(
			page.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม', exact: true })
		).toBeVisible();
		expect(overlay.reads).toHaveLength(0);
		await expect(toggle(page)).toHaveAttribute('aria-pressed', 'false');
		await toggle(page).click();
		await expect(toggle(page)).toHaveAttribute('aria-pressed', 'true');
		const day = page.getByRole('button', {
			name: '1 ต.ค. 2569, 1 กิจกรรม, 2 คำร้องรออนุมัติ',
			exact: true
		});
		await day.click();
		const selected = page.getByRole('region', { name: 'คำร้องในวันที่เลือก', exact: true });
		await expect(selected).toContainText('คำร้องเวลาเช้า');
		await expect(selected).toContainText('08:00 – 09:00');
		await expect(selected).toContainText('คำร้องหลายวัน');
		await expect(selected.getByRole('link', { name: 'เปิดคิวอนุมัติ' })).toHaveAttribute(
			'href',
			'/staff/calendar/requests?review=true&status=pending'
		);
		expect(overlay.reads[0].searchParams.get('from')).toBe('2026-09-27');
		expect(overlay.reads[0].searchParams.get('to')).toBe('2026-11-07');
		expect(api.count(eventsPath)).toBe(1);
		expect(api.writes).toHaveLength(0);
		await expect(page.getByRole('button', { name: 'แก้ไข กิจกรรมแรก', exact: true })).toHaveCount(
			1
		);
		await page.screenshot({
			path: testInfo.outputPath(`pending-${width}-light.png`),
			fullPage: true
		});
		await page.addStyleTag({
			content: '*,*::before,*::after {transition:none!important;animation:none!important}'
		});
		await page.evaluate(() => document.documentElement.classList.add('dark'));
		await page.screenshot({
			path: testInfo.outputPath(`pending-${width}-dark.png`),
			fullPage: true
		});
		expect(
			await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
		).toBe(true);
		await page
			.getByRole('button', { name: '2 ต.ค. 2569, 0 กิจกรรม, 1 คำร้องรออนุมัติ', exact: true })
			.click();
		await expect(selected).toContainText('คำร้องหลายวัน');
		await expect(selected).not.toContainText('คำร้องเวลาเช้า');
		await toggle(page).click();
		await expect(selected).toHaveCount(0);
		await expect(
			page.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม', exact: true })
		).toBeVisible();
		expect(overlay.reads).toHaveLength(1);
		expect(api.count(eventsPath)).toBe(1);
	});
}

test('switching months supersedes a delayed pending read and browser history restores the matching month', async ({
	page
}) => {
	await mockCalendar(page);
	const overlay = await mockOverlay(page, { holdFirst: true });
	await page.goto(calendarPath());
	await toggle(page).click();
	await expect.poll(() => overlay.reads.length).toBe(1);
	await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
	await expect(
		page.getByRole('button', { name: '10 พ.ย. 2569, 0 กิจกรรม, 1 คำร้องรออนุมัติ', exact: true })
	).toBeVisible();
	overlay.release();
	await expect(page.getByText('รออนุมัติ: คำร้องเวลาเช้า', { exact: true })).toHaveCount(0);
	await page.goBack();
	await expect(
		page.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม, 2 คำร้องรออนุมัติ', exact: true })
	).toBeVisible();
});

test('switching the overlay off cancels an in-flight read without adding pending markers later', async ({
	page
}) => {
	await mockCalendar(page);
	const overlay = await mockOverlay(page, { holdFirst: true });
	await page.goto(calendarPath());
	await toggle(page).click();
	await expect.poll(() => overlay.reads.length).toBe(1);
	await toggle(page).click();
	overlay.release();
	await expect(toggle(page)).toHaveAttribute('aria-pressed', 'false');
	await expect(
		page.getByRole('region', { name: 'คำร้องรออนุมัติบนปฏิทิน', exact: true })
	).toHaveCount(0);
	await expect(
		page.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม', exact: true })
	).toBeVisible();
});

test('pending failures and overflow stay local while confirmed events remain usable', async ({
	page
}) => {
	const api = await mockCalendar(page);
	const overlay = await mockOverlay(page, { failFirst: true, hasMore: true });
	await page.goto(calendarPath());
	await toggle(page).click();
	const region = page.getByRole('region', { name: 'คำร้องรออนุมัติบนปฏิทิน', exact: true });
	await expect(region).toContainText('คำร้องไม่พร้อม ลองใหม่');
	await expect(
		page.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม', exact: true })
	).toBeEnabled();
	await region.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(
		page.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม, 2 คำร้องรออนุมัติ', exact: true })
	).toBeVisible();
	await expect(region).toContainText('แสดงบนปฏิทินเพียง 500 รายการแรก');
	await expect.poll(() => overlay.reads.length).toBe(2);
	await expect.poll(() => api.count(eventsPath)).toBe(1);
});

test('read access alone never exposes the pending overlay', async ({ page }) => {
	await mockCalendar(page, { permissions: ['calendar.read.school'] });
	const overlay = await mockOverlay(page);
	await page.goto(calendarPath());
	await expect(
		page.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม', exact: true })
	).toBeVisible();
	await expect(toggle(page)).toHaveCount(0);
	expect(overlay.reads).toHaveLength(0);
});

test('the oldest-first queue does not skip the next waiting request after a decision removes a row', async ({
	page
}) => {
	await mockCalendar(page);
	const records: CalendarEventRequest[] = Array.from({ length: 32 }, (_, index) => ({
		id: id(200 + index),
		requestedBy: id(10),
		requesterName: 'ครูทดสอบ',
		title: `คำร้อง ${String(index + 1).padStart(2, '0')}`,
		description: 'รายละเอียดทดสอบ',
		location: null,
		startDate: '2026-10-01',
		endDate: '2026-10-01',
		allDay: true,
		startTime: null,
		endTime: null,
		status: 'pending',
		reviewedAt: null,
		rejectionReason: null,
		eventId: null,
		createdAt: new Date(Date.UTC(2026, 9, 1, 0, index)).toISOString()
	}));
	await page.route(
		(url) => url.pathname.startsWith('/api/calendar/requests'),
		(route) => {
			const request = route.request();
			const url = new URL(request.url());
			let data;
			if (request.method() === 'GET') {
				const offset = Number(url.searchParams.get('offset') ?? 0);
				const waiting = records.filter((item) => item.status === 'pending');
				data = {
					records: waiting.slice(offset, offset + 25),
					hasMore: waiting.length > offset + 25
				};
			} else {
				const record = records.find((item) => url.pathname.includes(item.id))!;
				record.status = 'rejected';
				record.rejectionReason = request.postDataJSON().reason;
				record.reviewedAt = '2026-10-01T02:00:00Z';
				data = record;
			}
			return route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({ success: true, data })
			});
		}
	);
	await page.goto('/staff/calendar/requests?review=true&status=pending');
	const queue = page.getByTestId('calendar-requests');
	await expect(queue.getByRole('heading', { level: 2 }).first()).toHaveText('คำร้อง 01');
	await queue.getByRole('button', { name: 'ไม่อนุมัติ', exact: true }).first().click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('textbox').fill('วันที่ซ้ำ');
	await dialog.getByRole('button', { name: 'บันทึกผลไม่อนุมัติ', exact: true }).click();
	await expect(queue.getByRole('heading', { level: 2 }).first()).toHaveText('คำร้อง 02');
	await queue.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(queue.getByRole('heading', { level: 2 }).first()).toHaveText('คำร้อง 26');
});

test('own pending requests and a successful submission patch markers without rereading confirmed events', async ({
	page
}) => {
	const api = await mockCalendar(page, {
		permissions: ['calendar.read.school', 'calendar.request.own']
	});
	const overlay = await mockOverlay(page, { records: [] });
	await page.route(
		(url) => url.pathname === '/api/calendar/requests',
		(route) => {
			const payload = route.request().postDataJSON();
			const request: CalendarEventRequest = {
				...payload,
				id: id(142),
				requestedBy: id(10),
				requesterName: 'ครูทดสอบ',
				status: 'pending',
				reviewedAt: null,
				rejectionReason: null,
				eventId: null,
				createdAt: '2026-10-01T02:00:00Z'
			};
			return route.fulfill({
				status: 201,
				contentType: 'application/json',
				body: JSON.stringify({ success: true, data: request })
			});
		}
	);
	await page.goto(calendarPath());
	await toggle(page).click();
	await expect(
		page.getByRole('region', { name: 'คำร้องรออนุมัติบนปฏิทิน', exact: true })
	).toContainText('ของฉัน');
	await expect(
		page.getByText('ไม่มีคำร้องรออนุมัติในช่วงวันที่นี้', { exact: true })
	).toBeVisible();
	await page.getByRole('button', { name: 'คำร้องขอเพิ่มวันกิจกรรม', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('ชื่อกิจกรรม *', { exact: true }).fill('คำร้องของฉันใหม่');
	await dialog.getByLabel('รายละเอียดกิจกรรม *', { exact: true }).fill('รายละเอียดทดสอบ');
	await dialog.getByRole('button', { name: 'ส่งคำร้อง', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	await expect(
		page.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม, 1 คำร้องรออนุมัติ', exact: true })
	).toBeVisible();
	await expect(
		page.getByRole('region', { name: 'คำร้องในวันที่เลือก', exact: true })
	).toContainText('คำร้องของฉันใหม่');
	expect(overlay.reads).toHaveLength(1);
	expect(api.count(eventsPath)).toBe(1);
	expect(api.writes).toHaveLength(0);
});
