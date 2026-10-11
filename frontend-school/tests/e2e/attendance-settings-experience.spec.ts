import { test, expect, type Page, type Route } from '@playwright/test';
import { mockStaffHome, id, year, actor } from './fixtures/staff-home-route-data';
import type { AttendanceSettings, AttendanceOptions } from '../../src/lib/api/attendance';
import type { CalendarViewerEvent } from '../../src/lib/api/calendar';
const term = id(200),
	date = '2026-10-09',
	student = id(202);
const path = `/staff/attendance/settings?academicYearId=${year}&academicTermId=${term}&date=${date}`;
const reply = (route: Route, data: unknown, status = 200) =>
	route.fulfill({
		status,
		contentType: 'application/json',
		headers: { 'X-CSRF-Token': 'synthetic-csrf' },
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
const settings: AttendanceSettings = {
	academicTermId: term,
	archived: false,
	rowVersion: 1,
	configuration: {
		enabled: true,
		weekdays: [1, 2, 3, 4, 5],
		lateAfter: '08:00:00',
		digestTimes: ['16:00:00'],
		evidenceDays: 30,
		faceDistance: 0.45,
		faceMargin: 0.1,
		activityCountsAsPresent: true
	}
};
const event: CalendarViewerEvent = {
	id: id(500),
	title: 'วันหยุดโรงเรียน',
	description: 'ประกอบการตัดสินใจเลือกวันนับยอด',
	startDate: date,
	endDate: '2026-10-10',
	startTime: null,
	endTime: null,
	allDay: true,
	categoryId: null,
	categoryColor: '#dc2626',
	categoryName: null,
	location: 'โรงเรียน',
	tags: [],
	isPublic: true,
	createdAt: '2026-10-01T00:00:00Z',
	updatedAt: '2026-10-01T00:00:00Z'
};
async function fixture(page: Page, schoolCalendar = true, archived = false) {
	await mockStaffHome(page, {
		permissions: [
			'attendance.manage.school',
			'academic_context.read.school',
			...(schoolCalendar ? ['calendar.read.school'] : [])
		]
	});
	await page.clock.setFixedTime(new Date('2026-10-09T01:00:00Z'));
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ contentType: 'text/css', body: '' })
	);
	await page.route(
		(url) => url.pathname === '/api/academic/context/options',
		(route) =>
			reply(route, {
				activeAcademicYearId: year,
				activeAcademicTermId: term,
				years: [
					{
						id: year,
						name: '2569',
						year: 2569,
						status: 'active',
						startDate: '2026-05-01',
						plannedEndDate: '2027-04-30',
						closedOn: null
					}
				],
				terms: [
					{
						id: term,
						academicYearId: year,
						code: '1',
						name: 'ภาคเรียนที่ 1',
						sequence: 1,
						termType: 'regular',
						status: 'active',
						startDate: '2026-05-01',
						plannedEndDate: '2026-10-31',
						closedOn: null,
						includedInYearResult: true,
						blocksYearClosure: true
					}
				]
			})
	);
	let saved = structuredClone(settings);
	saved.archived = archived;
	const options: AttendanceOptions = {
		students: [{ id: student, name: 'นักเรียนทดสอบ', homeroomId: id(201), homeroomName: 'ม.1/1' }],
		teachers: [{ id: actor, name: 'ครูทดสอบ' }],
		audiences: [],
		devices: [],
		specials: []
	};
	const reads: string[] = [],
		writes: { path: string; data: unknown }[] = [];
	await page.route(
		(url) =>
			url.pathname.startsWith('/api/attendance/') ||
			['/api/calendar/events', '/api/me/calendar/events'].includes(url.pathname),
		async (route) => {
			const request = route.request(),
				url = new URL(request.url());
			if (request.method() === 'GET') reads.push(url.pathname);
			else writes.push({ path: url.pathname, data: request.postDataJSON() });
			if (url.pathname.includes('/settings/')) {
				if (request.method() !== 'GET')
					saved = { ...saved, ...request.postDataJSON(), rowVersion: saved.rowVersion + 1 };
				return reply(route, saved);
			}
			if (url.pathname === '/api/attendance/days') return reply(route, []);
			if (url.pathname.includes('/days/')) {
				saved.rowVersion++;
				return reply(route, saved);
			}
			if (url.pathname.endsWith('/options')) return reply(route, options);
			if (url.pathname.endsWith('/calendar/events'))
				return reply(route, [
					{ ...event, targets: [], reminders: [], createdBy: null, updatedBy: null }
				]);
			if (url.pathname.includes('/audiences/')) return reply(route, 'บันทึกกลุ่มไม่พร้อม', 409);
			return reply(route, {});
		}
	);
	return { reads, writes };
}

test('24-hour composed inputs normalize compact times and preserve failures', async ({ page }) => {
	const api = await fixture(page);
	await page.goto(path);
	await expect(page.getByLabel('เข้าสายหลังเวลา')).toHaveValue('08:00');
	await expect(page.locator('input[type=time], input[type=month], input[type=date]')).toHaveCount(
		0
	);
	await page.getByLabel('เข้าสายหลังเวลา').fill('0830');
	await page.getByLabel('เวลาสรุป 1', { exact: true }).fill('1745');
	await page.getByRole('button', { name: 'บันทึกตั้งค่า', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	expect(api.writes[0].data).toMatchObject({
		configuration: { lateAfter: '08:30:00', digestTimes: ['17:45:00'] }
	});
	await page.getByLabel('เข้าสายหลังเวลา').fill('25:00');
	await page.getByRole('button', { name: 'บันทึกตั้งค่า', exact: true }).click();
	await expect(page.getByText('ระบุเวลาเข้าสายให้ถูกต้อง', { exact: true })).toBeVisible();
	expect(api.writes.length).toBe(1);
	await expect(page.getByLabel('เข้าสายหลังเวลา')).toHaveValue('25:00');
});

test('real school events inform explicit day decisions without automatic counting', async ({
	page
}) => {
	const api = await fixture(page);
	await page.goto(path);
	await expect(
		page.getByRole('region', { name: 'วันที่เลือก' }).getByText('วันหยุดโรงเรียน', { exact: true })
	).toBeVisible();
	await expect(page.getByRole('button', { name: 'นับยอดวันนี้ ✓', exact: true })).toBeEnabled();
	expect(api.writes).toHaveLength(0);
	expect(api.reads).toContain('/api/calendar/events');
	await page.getByRole('button', { name: 'นับยอดวันนี้ ✓', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ไม่นับยอดวันนี้ —', exact: true })).toBeVisible();
	expect(api.writes[0].data).toMatchObject({ days: [{ date, counted: false }] });
});

test('calendar failures and focused retry leave day editing usable', async ({ page }) => {
	const api = await fixture(page, false);
	let fail = true;
	await page.route(
		(url) => url.pathname === '/api/me/calendar/events',
		async (route) => {
			if (fail) return reply(route, 'ปฏิทินไม่พร้อม', 503);
			return route.fallback();
		}
	);
	await page.goto(path);
	await expect(page.getByText('ปฏิทินไม่พร้อม', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'นับยอดวันนี้ ✓', exact: true })).toBeEnabled();
	const attendanceReads = api.reads.filter((value) => value.startsWith('/api/attendance')).length;
	fail = false;
	await page.getByRole('button', { name: 'ลองโหลดกิจกรรมใหม่' }).click();
	await expect(
		page.getByRole('region', { name: 'วันที่เลือก' }).getByText('วันหยุดโรงเรียน', { exact: true })
	).toBeVisible();
	expect(api.reads).not.toContain('/api/calendar/events');
	expect(api.reads.filter((value) => value.startsWith('/api/attendance')).length).toBe(
		attendanceReads
	);
});

test('group dialog is lazy and keeps local failed draft while special session remains visible', async ({
	page
}) => {
	await fixture(page);
	await page.goto(path);
	await expect(page.getByLabel('ชื่อกลุ่ม', { exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: 'จัดการกลุ่ม', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('ชื่อกลุ่ม', { exact: true }).fill('ชุมนุมวิทย์');
	await dialog.getByRole('button', { name: 'เพิ่มกลุ่ม', exact: true }).click();
	await expect(dialog.getByText('เลือกสมาชิกอย่างน้อยหนึ่งคน', { exact: true })).toBeVisible();
	await dialog.getByRole('button', { name: 'สมาชิกกลุ่ม', exact: true }).click();
	await page.getByRole('option', { name: 'ม.1/1 · นักเรียนทดสอบ', exact: true }).click();
	await page.keyboard.press('Escape');
	await dialog.getByRole('button', { name: 'เพิ่มกลุ่ม', exact: true }).click();
	await expect(dialog.getByText('บันทึกกลุ่มไม่พร้อม', { exact: true })).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(dialog).toHaveCount(0);
	await page.getByLabel('ชื่อการเช็คชื่อ', { exact: true }).fill('กิจกรรมที่ยังไม่บันทึก');
	await page.getByRole('button', { name: 'จัดการกลุ่ม', exact: true }).click();
	await expect(dialog.getByLabel('ชื่อกลุ่ม', { exact: true })).toHaveValue('ชุมนุมวิทย์');
	await expect(dialog.getByText('บันทึกกลุ่มไม่พร้อม', { exact: true })).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(page.getByLabel('ชื่อการเช็คชื่อ', { exact: true })).toHaveValue(
		'กิจกรรมที่ยังไม่บันทึก'
	);
});

test('month navigation supersedes slow day and calendar reads without writes', async ({ page }) => {
	const api = await fixture(page);
	await page.goto(path);
	await expect(page.getByRole('button', { name: 'นับยอดวันนี้ ✓', exact: true })).toBeEnabled();
	let release!: () => void;
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	await page.route(
		(url) =>
			['/api/attendance/days', '/api/calendar/events'].includes(url.pathname) &&
			(url.searchParams.get('start') ?? url.searchParams.get('from'))?.startsWith('2026-11') ===
				true,
		async (route) => {
			await held;
			await reply(route, []);
		}
	);
	await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
	await expect(page.locator('[data-calendar-date="2026-11-15"]')).toBeVisible();
	await expect(page.getByRole('button', { name: 'นับช่วงนี้', exact: true })).toBeDisabled();
	await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
	await expect(page.locator('[data-calendar-date="2026-12-15"]')).toBeVisible();
	await expect(page.getByRole('button', { name: 'นับช่วงนี้', exact: true })).toBeEnabled();
	release();
	await expect(page.locator('[data-calendar-date="2026-12-15"]')).toBeVisible();
	expect(api.writes).toHaveLength(0);
});

for (const viewport of [
	{ name: 'mobile', width: 390, height: 844 },
	{ name: 'desktop', width: 1440, height: 900 }
]) {
	for (const theme of ['light', 'dark'] as const) {
		test(`settings experience ${viewport.name} ${theme}`, async ({ page }) => {
			await page.setViewportSize(viewport);
			await page.emulateMedia({ colorScheme: theme });
			await fixture(page);
			await page.goto(path);
			await expect(
				page.getByRole('heading', { name: 'ปฏิทินวันประมวลผล', exact: true })
			).toBeVisible();
			await expect(
				page
					.getByRole('region', { name: 'วันที่เลือก' })
					.getByText('วันหยุดโรงเรียน', { exact: true })
			).toBeVisible();
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await page
				.getByRole('heading', { name: 'ปฏิทินวันประมวลผล', exact: true })
				.evaluate((element) => element.scrollIntoView({ block: 'start' }));
			await page.screenshot({
				animations: 'disabled',
				path: `/tmp/attendance-settings-${viewport.name}-${theme}.png`
			});
			await page.getByRole('button', { name: 'จัดการกลุ่ม', exact: true }).click();
			await expect(page.getByRole('dialog')).toBeVisible();
			await page.screenshot({
				animations: 'disabled',
				path: `/tmp/attendance-settings-groups-${viewport.name}-${theme}.png`
			});
			await page.keyboard.press('Escape');
			await expect(page.getByRole('dialog')).toHaveCount(0);
			await expect(page.getByRole('button', { name: 'จัดการกลุ่ม', exact: true })).toBeFocused();
		});
	}
}
