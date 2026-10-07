import { test, expect, type Page, type Route } from '@playwright/test';
import {
	mockCalendar,
	calendarPath,
	eventsPath,
	makeApprovedCalendarEvent
} from './fixtures/calendar-route-data';
import { id } from './fixtures/staff-home-route-data';
import type {
	PendingCalendarRequest,
	CalendarEventRequest,
	CreateCalendarEventRequest
} from '../../src/lib/api/calendar';

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
		await day.click({ position: { x: 5, y: 5 } });
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
			.click({ position: { x: 5, y: 5 } });
		await expect(selected).toContainText('คำร้องหลายวัน');
		await expect(selected).not.toContainText('คำร้องเวลาเช้า');
		await page.keyboard.press('Escape');
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
	await expect(page.getByRole('region', { name: 'คำร้องในวันที่เลือก', exact: true })).toHaveCount(
		0
	);
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
	await page
		.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม', exact: true })
		.click({ position: { x: 5, y: 5 } });
	const region = page.getByRole('region', { name: 'คำร้องในวันที่เลือก', exact: true });
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
	await page
		.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม', exact: true })
		.click({ position: { x: 5, y: 5 } });
	await expect(
		page.getByRole('region', { name: 'คำร้องในวันที่เลือก', exact: true })
	).toContainText('คำร้องของฉัน');
	await expect(page.getByText('วันนี้ไม่มีคำร้องรออนุมัติ', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'คำร้องขอเพิ่มวันกิจกรรม', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('ชื่อกิจกรรม *', { exact: true }).fill('คำร้องของฉันใหม่');
	await dialog.getByLabel('รายละเอียดกิจกรรม *', { exact: true }).fill('รายละเอียดทดสอบ');
	await dialog.getByRole('checkbox', { name: 'ทั้งวัน', exact: true }).check();
	await dialog.getByRole('button', { name: 'ส่งคำร้อง', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	await expect(
		page.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม, 1 คำร้องรออนุมัติ', exact: true })
	).toBeVisible();
	await page.getByRole('button', { name: /^ดูรายละเอียด คำร้องของฉันใหม่,/ }).click();
	await expect(
		page.getByRole('region', { name: 'คำร้องในวันที่เลือก', exact: true })
	).toContainText('คำร้องของฉันใหม่');
	expect(overlay.reads).toHaveLength(1);
	expect(api.count(eventsPath)).toBe(1);
	expect(api.writes).toHaveLength(0);
});

async function mockInlineDecisions(
	page: Page,
	options: { failDetail?: boolean; failDecision?: boolean; holdDetail?: boolean } = {}
) {
	const writes: { path: string; payload: CreateCalendarEventRequest | { reason: string } }[] = [];
	const details: string[] = [];
	let release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route(
		(url) => /^\/api\/calendar\/requests\/[0-9a-f-]+(?:\/(?:approve|reject))?$/.test(url.pathname),
		async (route) => {
			const req = route.request(),
				path = new URL(req.url()).pathname;
			const seed = pending.find((item) => path.includes(item.id))!;
			const detail: CalendarEventRequest = {
				...seed,
				requestedBy: id(10),
				requesterName: 'ครูทดสอบ',
				description: 'รายละเอียดคำร้องสำหรับพิจารณา',
				location: 'หอประชุม',
				status: 'pending',
				reviewedAt: null,
				rejectionReason: null,
				eventId: null,
				createdAt: '2026-10-01T02:00:00Z'
			};
			if (req.method() === 'GET') {
				details.push(path);
				if (options.holdDetail && details.length === 1) await held;
				if (options.failDetail && details.length === 1)
					return route.fulfill({
						status: 503,
						contentType: 'application/json',
						body: JSON.stringify({ success: false, error: 'อ่านคำร้องไม่สำเร็จ' })
					});
				return route.fulfill({
					contentType: 'application/json',
					body: JSON.stringify({ success: true, data: detail })
				});
			}
			const payload = req.postDataJSON();
			writes.push({ path, payload });
			if (options.failDecision && writes.length === 1)
				return route.fulfill({
					status: 503,
					contentType: 'application/json',
					body: JSON.stringify({ success: false, error: 'บันทึกไม่สำเร็จ ลองอีกครั้ง' })
				});
			return route.fulfill({
				contentType: 'application/json',
				body: JSON.stringify({
					success: true,
					data: path.endsWith('/approve')
						? {
								request: { ...detail, status: 'approved', eventId: id(145) },
								event: makeApprovedCalendarEvent(payload)
							}
						: { ...detail, status: 'rejected', rejectionReason: payload.reason }
				})
			});
		}
	);
	return { writes, details, release };
}
for (const width of [1280, 375]) {
	test(`approve a calendar request in place with full details and a retained failed draft at ${width}px`, async ({
		page
	}, testInfo) => {
		await page.setViewportSize({ width, height: 900 });
		const api = await mockCalendar(page);
		const overlay = await mockOverlay(page);
		const decisions = await mockInlineDecisions(page, { failDecision: true });
		await page.goto(calendarPath());
		await toggle(page).click();
		await page.getByRole('button', { name: /^ดูรายละเอียด คำร้องเวลาเช้า,/ }).click();
		const region = page.getByRole('region', { name: 'คำร้องในวันที่เลือก', exact: true });
		expect(decisions.details).toHaveLength(0);
		expect(api.count('/api/calendar/target-options')).toBe(0);
		const card = region
			.getByRole('article')
			.filter({ has: page.getByRole('heading', { name: 'คำร้องเวลาเช้า', exact: true }) });
		await card.getByRole('button', { name: 'ตรวจและอนุมัติ', exact: true }).click();
		const dialog = page.getByRole('dialog');
		await expect(dialog.getByLabel('ชื่อกิจกรรม', { exact: true })).toHaveValue('คำร้องเวลาเช้า');
		await expect(dialog.getByLabel('รายละเอียด', { exact: true })).toHaveValue(
			'รายละเอียดคำร้องสำหรับพิจารณา'
		);
		await expect(dialog.getByLabel('สถานที่', { exact: true })).toHaveValue('หอประชุม');
		await dialog.getByLabel('ชื่อกิจกรรม', { exact: true }).fill('กิจกรรมที่อนุมัติ');
		await dialog.getByRole('button', { name: 'อนุมัติและเพิ่มลงปฏิทิน', exact: true }).click();
		await expect(dialog.getByRole('alert')).toContainText('บันทึกไม่สำเร็จ');
		await expect(dialog.getByLabel('ชื่อกิจกรรม', { exact: true })).toHaveValue(
			'กิจกรรมที่อนุมัติ'
		);
		await page.screenshot({ path: testInfo.outputPath(`inline-approval-${width}-light.png`) });
		await page.addStyleTag({
			content: '*,*::before,*::after {transition:none!important;animation:none!important}'
		});
		await page.evaluate(() => document.documentElement.classList.add('dark'));
		await page.screenshot({ path: testInfo.outputPath(`inline-approval-${width}-dark.png`) });
		await dialog.getByRole('button', { name: 'อนุมัติและเพิ่มลงปฏิทิน', exact: true }).click();
		await expect(dialog).toHaveCount(0);
		await expect(region.getByRole('heading', { name: 'คำร้องเวลาเช้า', exact: true })).toHaveCount(
			0
		);
		await page
			.getByRole('button', { name: '1 ต.ค. 2569, 2 กิจกรรม, 1 คำร้องรออนุมัติ', exact: true })
			.click({ position: { x: 5, y: 5 } });
		await expect(region).toContainText('คำร้องหลายวัน');
		await expect(
			page.getByRole('button', { name: '1 ต.ค. 2569, 2 กิจกรรม, 1 คำร้องรออนุมัติ', exact: true })
		).toBeVisible();
		expect(decisions.details).toHaveLength(1);
		expect(decisions.writes).toHaveLength(2);
		expect(decisions.writes[1].path).toBe(`/api/calendar/requests/${pending[0].id}/approve`);
		expect(api.count(eventsPath)).toBe(1);
		expect(overlay.reads).toHaveLength(1);
		expect(api.writes).toHaveLength(0);
	});
	test(`reject a calendar request in place with a required reason at ${width}px`, async ({
		page
	}) => {
		await page.setViewportSize({ width, height: 900 });
		const api = await mockCalendar(page);
		const overlay = await mockOverlay(page);
		const decisions = await mockInlineDecisions(page, { failDecision: true });
		await page.goto(calendarPath());
		await toggle(page).click();
		await page
			.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม, 2 คำร้องรออนุมัติ', exact: true })
			.click({ position: { x: 5, y: 5 } });
		const region = page.getByRole('region', { name: 'คำร้องในวันที่เลือก', exact: true });
		await region
			.getByRole('article')
			.filter({ has: page.getByRole('heading', { name: 'คำร้องเวลาเช้า', exact: true }) })
			.getByRole('button', { name: 'ไม่อนุมัติ', exact: true })
			.click();
		const dialog = page.getByRole('dialog');
		await dialog.getByRole('button', { name: 'บันทึกผลไม่อนุมัติ', exact: true }).click();
		expect(decisions.writes).toHaveLength(0);
		await dialog.getByRole('textbox').fill('วันที่ซ้ำกับกิจกรรมอื่น');
		await dialog.getByRole('button', { name: 'บันทึกผลไม่อนุมัติ', exact: true }).click();
		await expect(dialog.getByRole('alert')).toContainText('บันทึกไม่สำเร็จ');
		await expect(dialog.getByRole('textbox')).toHaveValue('วันที่ซ้ำกับกิจกรรมอื่น');
		await dialog.getByRole('button', { name: 'บันทึกผลไม่อนุมัติ', exact: true }).click();
		await expect(dialog).toHaveCount(0);
		await expect(region.getByRole('heading', { name: 'คำร้องเวลาเช้า', exact: true })).toHaveCount(
			0
		);
		await expect(
			page.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม, 1 คำร้องรออนุมัติ', exact: true })
		).toBeVisible();
		expect(decisions.details).toHaveLength(0);
		expect(decisions.writes[1].payload).toEqual({ reason: 'วันที่ซ้ำกับกิจกรรมอื่น' });
		expect(overlay.reads).toHaveLength(1);
		expect(api.count(eventsPath)).toBe(1);
		expect(api.count('/api/calendar/target-options')).toBe(0);
		expect(api.writes).toHaveLength(0);
	});
	test(`compact calendar filters open next to the pending toggle without the removed notices at ${width}px`, async ({
		page
	}, testInfo) => {
		await page.setViewportSize({ width, height: 900 });
		const api = await mockCalendar(page);
		await mockOverlay(page, { records: [] });
		await page.goto(calendarPath());
		await toggle(page).click();
		await expect(page.getByText(/แถบเส้นประคือคำร้อง/)).toHaveCount(0);
		await expect(
			page.getByText('ไม่มีคำร้องรออนุมัติในช่วงวันที่นี้', { exact: true })
		).toHaveCount(0);
		const filter = page.getByRole('button', { name: 'เปิดตัวกรองปฏิทิน', exact: true });
		await expect(page.getByPlaceholder('ค้นหาชื่อ รายละเอียด สถานที่ หรือแท็ก')).toHaveCount(0);
		const a = await toggle(page).boundingBox(),
			b = await filter.boundingBox();
		if (width === 1280) {
			expect(a!.y).toBe(b!.y);
			expect(b!.x).toBeGreaterThan(a!.x);
		}
		await filter.click();
		const form = page.getByRole('form', { name: 'ตัวกรองปฏิทิน', exact: true });
		await expect(form).toContainText('ทุกหมวดหมู่');
		await expect(form).toContainText('ทุกแท็ก');
		await expect(form).toContainText('ทุกกลุ่มผู้ชม');
		await expect(form).toContainText('ทุกสถานะ');
		await page.screenshot({ path: testInfo.outputPath(`filters-${width}-light.png`) });
		await page.addStyleTag({
			content: '*,*::before,*::after {transition:none!important;animation:none!important}'
		});
		await page.evaluate(() => document.documentElement.classList.add('dark'));
		await page.screenshot({ path: testInfo.outputPath(`filters-${width}-dark.png`) });
		await form.getByPlaceholder('ค้นหาชื่อ รายละเอียด สถานที่ หรือแท็ก').fill('ไม่มีรายการ');
		await form.getByRole('button', { name: /^กรอง(?: \d+)?$/ }).click();
		await expect(form).toHaveCount(0);
		await expect(page).toHaveURL(/q=/);
		await expect(page.getByTestId('calendar-events')).toContainText('ยังไม่มีกิจกรรม');
		expect(api.count('/api/calendar/categories')).toBe(1);
		expect(api.count('/api/calendar/tags')).toBe(1);
		expect(
			await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
		).toBe(true);
	});
}
test('request detail retry stays local and closing a delayed review cannot replace the next draft', async ({
	page
}) => {
	const api = await mockCalendar(page);
	await mockOverlay(page);
	const decisions = await mockInlineDecisions(page, { holdDetail: true });
	await page.goto(calendarPath());
	await toggle(page).click();
	await page
		.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม, 2 คำร้องรออนุมัติ', exact: true })
		.click({ position: { x: 5, y: 5 } });
	const region = page.getByRole('region', { name: 'คำร้องในวันที่เลือก', exact: true });
	await region.getByRole('button', { name: 'ตรวจและอนุมัติ', exact: true }).first().click();
	await expect(page.getByRole('dialog').getByRole('status')).toBeVisible();
	await page.keyboard.press('Escape');
	await page
		.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม, 2 คำร้องรออนุมัติ', exact: true })
		.click({ position: { x: 5, y: 5 } });
	await region.getByRole('button', { name: 'ตรวจและอนุมัติ', exact: true }).last().click();
	await expect(page.getByLabel('ชื่อกิจกรรม', { exact: true })).toHaveValue('คำร้องหลายวัน');
	decisions.release();
	await expect(page.getByLabel('ชื่อกิจกรรม', { exact: true })).toHaveValue('คำร้องหลายวัน');
	expect(api.count(eventsPath)).toBe(1);
});
test('calendar review details can fail and retry without affecting event or pending reads', async ({
	page
}) => {
	const api = await mockCalendar(page);
	const overlay = await mockOverlay(page);
	const decisions = await mockInlineDecisions(page, { failDetail: true });
	await page.goto(calendarPath());
	await toggle(page).click();
	await page
		.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม, 2 คำร้องรออนุมัติ', exact: true })
		.click({ position: { x: 5, y: 5 } });
	await page
		.getByRole('region', { name: 'คำร้องในวันที่เลือก', exact: true })
		.getByRole('button', { name: 'ตรวจและอนุมัติ', exact: true })
		.first()
		.click();
	const dialog = page.getByRole('dialog');
	await expect(dialog).toContainText('อ่านคำร้องไม่สำเร็จ');
	await dialog.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(dialog.getByLabel('ชื่อกิจกรรม', { exact: true })).toHaveValue('คำร้องเวลาเช้า');
	expect(decisions.details).toHaveLength(2);
	expect(api.count(eventsPath)).toBe(1);
	expect(overlay.reads).toHaveLength(1);
});
test('requesters see their request dates but no manager actions or detail reads', async ({
	page
}) => {
	await mockCalendar(page, { permissions: ['calendar.read.school', 'calendar.request.own'] });
	await mockOverlay(page);
	const decisions = await mockInlineDecisions(page);
	await page.goto(calendarPath());
	await toggle(page).click();
	await page
		.getByRole('button', { name: '1 ต.ค. 2569, 1 กิจกรรม, 2 คำร้องรออนุมัติ', exact: true })
		.click({ position: { x: 5, y: 5 } });
	const region = page.getByRole('region', { name: 'คำร้องในวันที่เลือก', exact: true });
	await expect(region).toContainText('คำร้องของฉัน');
	await expect(region.getByRole('button', { name: 'ตรวจและอนุมัติ', exact: true })).toHaveCount(0);
	await expect(region.getByRole('button', { name: 'ไม่อนุมัติ', exact: true })).toHaveCount(0);
	expect(decisions.details).toHaveLength(0);
	expect(decisions.writes).toHaveLength(0);
});
