import { test, expect, type Page, type Route } from '@playwright/test';
import {
	mockCalendar,
	calendarPath,
	nextYear,
	categoryId,
	eventId
} from './fixtures/calendar-route-data';
import type { CalendarEventRequest } from '../../src/lib/api/calendar';
import { id } from './fixtures/staff-home-route-data';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.clock.setFixedTime(new Date('2026-10-01T02:00:00Z'));
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
const pending: CalendarEventRequest = {
	id: id(120),
	requestedBy: id(10),
	requesterName: 'ครูทดสอบ',
	title: 'วันกิจกรรมเสนอ',
	description: 'รายละเอียดเพื่อพิจารณา',
	location: 'หอประชุม',
	startDate: '2027-06-12',
	endDate: '2027-06-12',
	allDay: true,
	startTime: null,
	endTime: null,
	status: 'pending',
	reviewedAt: null,
	rejectionReason: null,
	eventId: null,
	createdAt: '2026-10-01T00:00:00Z'
};
async function requests(page: Page, failFirst = false) {
	const writes: { path: string; payload: Record<string, unknown> }[] = [];
	let record: CalendarEventRequest = { ...pending };
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	await page.route('**/api/calendar/requests**', async (route) => {
		const req = route.request(),
			path = new URL(req.url()).pathname;
		if (req.method() === 'GET') return reply(route, { records: [record], hasMore: false });
		const payload = req.postDataJSON();
		writes.push({ path, payload });
		if (failFirst && writes.length === 1) return reply(route, 'ส่งไม่สำเร็จ ลองอีกครั้ง', 503);
		if (path.endsWith('/reject')) {
			record = { ...record, status: 'rejected', rejectionReason: payload.reason };
			return reply(route, record);
		}
		if (path.endsWith('/approve')) {
			record = { ...record, status: 'approved', eventId };
			return reply(route, { request: record, event: { id: eventId, ...payload } });
		}
		record = { ...record, ...payload };
		return reply(route, record, 201);
	});
	return writes;
}
for (const width of [1280, 375])
	test(`request draft follows selected day and survives failure at ${width}px`, async ({
		page
	}, testInfo) => {
		await page.setViewportSize({ width, height: 900 });
		const api = await mockCalendar(page, {
			permissions: ['calendar.read.school', 'calendar.request.own']
		});
		const writes = await requests(page, true);
		await page.goto(calendarPath(nextYear, '2027-06'));
		await page.getByRole('button', { name: '12 มิ.ย. 2570, 0 กิจกรรม', exact: true }).click();
		await page.getByRole('button', { name: 'คำร้องขอเพิ่มวันกิจกรรม', exact: true }).click();
		const dialog = page.getByRole('dialog');
		await expect(
			dialog.getByRole('button', { name: 'วันที่เริ่มกิจกรรม', exact: true })
		).toContainText('2570');
		await expect(dialog).not.toContainText('หมวดหมู่');
		await expect(dialog).not.toContainText('ปีการศึกษา');
		await dialog.getByLabel('ชื่อกิจกรรม *', { exact: true }).fill('วันกิจกรรมเสนอ');
		await dialog.getByLabel('รายละเอียดกิจกรรม *', { exact: true }).fill('รายละเอียดเพื่อพิจารณา');
		await dialog.getByRole('button', { name: 'ส่งคำร้อง', exact: true }).click();
		await expect(dialog.getByRole('alert')).toContainText('ส่งไม่สำเร็จ');
		await expect(dialog.getByLabel('ชื่อกิจกรรม *', { exact: true })).toHaveValue('วันกิจกรรมเสนอ');
		expect(writes[0].payload).toEqual({
			title: 'วันกิจกรรมเสนอ',
			description: 'รายละเอียดเพื่อพิจารณา',
			location: null,
			startDate: '2027-06-12',
			endDate: '2027-06-12',
			allDay: true,
			startTime: null,
			endTime: null
		});
		expect(
			await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
		).toBe(true);
		await page.keyboard.press('Tab');
		expect(await dialog.evaluate((el) => el.contains(document.activeElement))).toBe(true);
		await page.screenshot({ path: testInfo.outputPath(`request-${width}-light.png`) });
		await page.addStyleTag({
			content: '*, *::before, *::after { transition: none !important; animation: none !important; }'
		});
		await page.evaluate(() => document.documentElement.classList.add('dark'));
		await page.screenshot({ path: testInfo.outputPath(`request-${width}-dark.png`) });
		await dialog.getByRole('button', { name: 'ส่งคำร้อง', exact: true }).click();
		await expect(dialog).toHaveCount(0);
		expect(api.writes).toHaveLength(0);
		await page.getByRole('link', { name: 'คำร้องของฉัน', exact: true }).click();
		await expect(page.getByTestId('calendar-requests')).toContainText('รออนุมัติ');
		await expect(page.getByRole('button', { name: 'ตรวจและอนุมัติ', exact: true })).toHaveCount(0);
	});
test('direct event defaults to selected date despite an unrelated header year', async ({
	page
}) => {
	const api = await mockCalendar(page);
	await page.goto(calendarPath(nextYear, '2027-06'));
	await page.getByRole('button', { name: '12 มิ.ย. 2570, 0 กิจกรรม', exact: true }).click();
	await page.getByRole('button', { name: 'เพิ่มกิจกรรม', exact: true }).click();
	await page.getByLabel('ชื่อกิจกรรม', { exact: true }).fill('กิจกรรมในอนาคต');
	const posted = page.waitForRequest(
		(req) => new URL(req.url()).pathname === '/api/calendar/events' && req.method() === 'POST'
	);
	await page.getByRole('button', { name: 'บันทึกและเผยแพร่', exact: true }).click();
	const payload = (await posted).postDataJSON();
	expect(payload.startDate).toBe('2027-06-12');
	expect(payload.endDate).toBe('2027-06-12');
	expect(payload).not.toHaveProperty('academicYearId');
	await expect.poll(() => api.writes.length).toBe(1);
	expect(
		api.reads
			.find((url) => url.pathname === '/api/calendar/target-options')
			?.searchParams.get('date')
	).toBe('2027-06-12');
});
test('manager selects event settings before approval, with date-specific targets loaded lazily', async ({
	page
}) => {
	const api = await mockCalendar(page);
	const writes = await requests(page);
	await page.goto('/staff/calendar/requests?review=true');
	await expect(page.getByTestId('calendar-requests')).toContainText(pending.title);
	expect(api.count('/api/calendar/target-options')).toBe(0);
	await page.getByRole('button', { name: 'ตรวจและอนุมัติ', exact: true }).click();
	await expect(page.getByLabel('ชื่อกิจกรรม', { exact: true })).toHaveValue(pending.title);
	await page.getByRole('button', { name: 'ไม่ระบุหมวดหมู่', exact: true }).click();
	await page.getByRole('option', { name: 'หมวดแรก', exact: true }).click();
	await page.getByRole('button', { name: 'อนุมัติและเพิ่มลงปฏิทิน', exact: true }).click();
	await expect(page.getByTestId('calendar-requests')).toContainText('อนุมัติแล้ว');
	expect(writes[0].path).toBe(`/api/calendar/requests/${pending.id}/approve`);
	expect(writes[0].payload.startDate).toBe(pending.startDate);
	expect(writes[0].payload).not.toHaveProperty('academicYearId');
	expect(writes[0].payload).not.toHaveProperty('academicTermId');
	expect(writes[0].payload).toHaveProperty('targets');
	expect(writes[0].payload.categoryId).toBe(categoryId);
	expect(
		api.reads
			.find((url) => url.pathname === '/api/calendar/target-options')
			?.searchParams.get('date')
	).toBe(pending.startDate);
	expect(api.writes).toHaveLength(0);
});
test('rejection requires and displays a reason without creating an event', async ({ page }) => {
	const api = await mockCalendar(page);
	const writes = await requests(page);
	await page.goto('/staff/calendar/requests?review=true');
	await page.getByRole('button', { name: 'ไม่อนุมัติ', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('textbox').fill('ขอปรับกำหนดการ');
	await dialog.getByRole('button', { name: 'บันทึกผลไม่อนุมัติ', exact: true }).click();
	await expect(page.getByTestId('calendar-requests')).toContainText('ขอปรับกำหนดการ');
	expect(writes[0].payload).toEqual({ reason: 'ขอปรับกำหนดการ' });
	expect(api.writes).toHaveLength(0);
});

test('a requester cannot load the manager review queue', async ({ page }) => {
	await mockCalendar(page, { permissions: ['calendar.read.school', 'calendar.request.own'] });
	await requests(page);
	const reads: string[] = [];
	page.on('request', (request) => {
		if (new URL(request.url()).pathname === '/api/calendar/requests') reads.push(request.url());
	});
	await page.goto('/staff/calendar/requests?review=true');
	await expect(page.getByText('ไม่มีสิทธิ์ดูคำร้องนี้', { exact: true })).toBeVisible();
	expect(reads).toHaveLength(0);
});
