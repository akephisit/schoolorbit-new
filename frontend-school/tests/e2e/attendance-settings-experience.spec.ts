import { test, expect, type Page, type Route } from '@playwright/test';
import { mockStaffHome, id, year, actor } from './fixtures/staff-home-route-data';
import type {
	AttendanceSettings,
	AttendanceOptions,
	SaveAttendanceDays
} from '../../src/lib/api/attendance';
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
	autoFinalizeFrom: '2026-10-09',
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
				defaultAcademicYearId: year,
				defaultAcademicTermId: term,
				years: [
					{
						isVisible: true,
						id: year,
						name: '2569',
						year: 2569,
						status: 'active'
					}
				],
				terms: [
					{
						isVisible: true,
						id: term,
						academicYearId: year,
						code: '1',
						name: 'ภาคเรียนที่ 1',
						sequence: 1,
						termType: 'regular',
						status: 'active',
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
		page.getByRole('button', { name: /ดูรายละเอียด วันหยุดโรงเรียน/ }).first()
	).toBeVisible();
	await expect(page.locator(`[data-calendar-date="${date}"]`)).toBeEnabled();
	await expect(page.locator(`[data-calendar-date="${date}"]`)).toHaveAttribute(
		'aria-label',
		/1 กิจกรรม, ✓ นับยอด/
	);
	expect(api.writes).toHaveLength(0);
	expect(api.reads).toContain('/api/calendar/events');
	await expect(page.getByRole('region', { name: 'วันที่เลือก' })).toHaveCount(0);
	await page
		.getByRole('button', { name: /ดูรายละเอียด วันหยุดโรงเรียน/ })
		.first()
		.click();
	await expect(page.getByRole('dialog')).toContainText('ประกอบการตัดสินใจเลือกวันนับยอด');
	expect(api.writes).toHaveLength(0);
	await page.keyboard.press('Escape');
	await page.locator(`[data-calendar-date="${date}"]`).click();
	await expect(page.locator(`[data-calendar-date="${date}"]`)).toHaveAttribute(
		'aria-pressed',
		'false'
	);
	expect(api.writes[0].data).toMatchObject({ days: [{ date, counted: false }] });
	await page.locator(`[data-calendar-date="${date}"]`).focus();
	await page.keyboard.press('Space');
	await expect(page.locator(`[data-calendar-date="${date}"]`)).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await expect.poll(() => api.writes.length).toBe(2);
	expect(api.writes[1].data).toMatchObject({ rowVersion: 2, days: [{ date, counted: true }] });
});

test('day click stays usable while saving and retains pending selection after rejection', async ({
	page
}) => {
	const api = await fixture(page);
	let release!: () => void;
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	let attempts = 0;
	await page.route(
		(url) => url.pathname.includes('/api/attendance/days/'),
		async (route) => {
			attempts++;
			await held;
			await reply(route, 'การตั้งค่าถูกแก้ไขแล้ว กรุณาโหลดข้อมูลใหม่', 409);
		}
	);
	await page.goto(path);
	const cell = page.locator(`[data-calendar-date="${date}"]`);
	await expect(cell).toBeEnabled();
	await expect(cell).toHaveAttribute('aria-pressed', 'true');
	await expect(cell).toHaveText('9');
	await cell.scrollIntoViewIfNeeded();
	const dimensions = await cell.boundingBox();
	await cell.click({ position: { x: dimensions!.width * 0.7, y: dimensions!.height * 0.8 } });
	await expect(cell).toHaveAttribute('aria-pressed', 'false');
	await expect(cell).toHaveAttribute('aria-busy', 'true');
	await expect(cell).toBeEnabled();
	await expect(page.locator('[data-calendar-date="2026-10-12"]')).toBeEnabled();
	expect(attempts).toBe(1);
	const pendingDimensions = await cell.boundingBox();
	expect(pendingDimensions).toEqual(dimensions);
	release();
	await expect(
		page.getByText('การตั้งค่าถูกแก้ไขแล้ว กรุณาโหลดข้อมูลใหม่', { exact: true })
	).toBeVisible();
	await expect(cell).toBeEnabled();
	await expect(cell).toHaveAttribute('aria-pressed', 'false');
	await expect(cell).toHaveAttribute('aria-busy', 'true');
	await expect(
		page.getByRole('button', { name: 'โหลดล่าสุดแล้วลองใหม่', exact: true })
	).toBeEnabled();
	expect(attempts).toBe(1);
	expect(api.writes).toHaveLength(0);
});

test('rapid day edits coalesce and serialize batches without losing newer selections', async ({
	page
}) => {
	const api = await fixture(page);
	const gates: Array<() => void> = [];
	const holds = [0, 1].map(() => new Promise<void>((resolve) => gates.push(resolve)));
	const writes: SaveAttendanceDays[] = [];
	let active = 0,
		peak = 0;
	await page.route(
		(url) => url.pathname.includes('/api/attendance/days/'),
		async (route) => {
			const index = writes.length;
			writes.push(route.request().postDataJSON());
			active++;
			peak = Math.max(peak, active);
			await holds[index];
			active--;
			await reply(route, { ...settings, rowVersion: index + 2 });
		}
	);
	await page.goto(path);
	const first = page.locator(`[data-calendar-date="${date}"]`);
	await expect(first).toBeEnabled();
	await first.click();
	await expect.poll(() => writes.length).toBe(1);
	await page.locator('[data-calendar-date="2026-10-12"]').click();
	await page.locator('[data-calendar-date="2026-10-10"]').click();
	await first.click();
	await page.locator('[data-calendar-date="2026-10-11"]').click();
	await page.locator('[data-calendar-date="2026-10-11"]').click();
	await expect(first).toHaveAttribute('aria-pressed', 'true');
	expect(writes).toHaveLength(1);
	await expect(page.getByRole('button', { name: 'บันทึกตั้งค่า', exact: true })).toBeDisabled();
	await expect(page.getByRole('button', { name: 'เดือนถัดไป', exact: true })).toBeDisabled();
	await page.getByRole('link', { name: 'สรุป / ล้างภาคเรียน', exact: true }).click();
	await expect(page).toHaveURL(new RegExp('/attendance/settings\\?'));
	await expect(page.getByText(/รอให้ปฏิทินบันทึกเสร็จ/)).toBeVisible();
	gates[0]();
	await expect.poll(() => writes.length).toBe(2);
	expect(writes[0]).toMatchObject({ rowVersion: 1, days: [{ date, counted: false }] });
	expect(writes[1].rowVersion).toBe(2);
	expect(writes[1].days).toHaveLength(4);
	expect(writes[1].days).toEqual(
		expect.arrayContaining([
			expect.objectContaining({ date, counted: true }),
			expect.objectContaining({ date: '2026-10-12', counted: false }),
			expect.objectContaining({ date: '2026-10-10', counted: true }),
			expect.objectContaining({ date: '2026-10-11', counted: false })
		])
	);
	await expect(first).toHaveAttribute('aria-pressed', 'true');
	await expect(first).toHaveAttribute('aria-busy', 'true');
	gates[1]();
	await expect(first).not.toHaveAttribute('aria-busy', 'true');
	await expect(page.getByRole('button', { name: 'บันทึกตั้งค่า', exact: true })).toBeEnabled();
	expect(peak).toBe(1);
	await page.getByRole('button', { name: 'บันทึกตั้งค่า', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	expect(api.writes[0].data).toMatchObject({ rowVersion: 3 });
});

test('failed queue pauses automatic writes and retry reconciles only settings and dates', async ({
	page
}) => {
	const api = await fixture(page);
	const writes: SaveAttendanceDays[] = [];
	let releaseRetry!: () => void;
	const heldRetry = new Promise<void>((resolve) => {
		releaseRetry = resolve;
	});
	let conflicted = false,
		failReload = false;
	await page.route(
		(url) => url.pathname.includes('/api/attendance/days/'),
		async (route) => {
			writes.push(route.request().postDataJSON());
			if (writes.length === 1) {
				conflicted = true;
				return reply(route, 'การตั้งค่าถูกแก้ไขแล้ว กรุณาโหลดข้อมูลใหม่', 409);
			}
			if (writes.length === 2) await heldRetry;
			return reply(route, { ...settings, rowVersion: writes.length + 5 });
		}
	);
	await page.route(
		(url) => url.pathname.includes('/api/attendance/settings/'),
		async (route) => {
			if (route.request().method() !== 'GET') return route.fallback();
			if (failReload) return reply(route, 'โหลดข้อมูลล่าสุดไม่พร้อม', 503);
			return reply(route, { ...settings, rowVersion: conflicted ? 6 : 1 });
		}
	);
	await page.goto(path);
	await page.getByLabel('เข้าสายหลังเวลา').fill('0830');
	const first = page.locator(`[data-calendar-date="${date}"]`);
	await first.click();
	await expect(
		page.getByText('การตั้งค่าถูกแก้ไขแล้ว กรุณาโหลดข้อมูลใหม่', { exact: true })
	).toBeVisible();
	await page.locator('[data-calendar-date="2026-10-12"]').click();
	await first.click();
	expect(writes).toHaveLength(1);
	const otherReads = api.reads.filter(
		(value) => value !== '/api/attendance/days' && !value.includes('/settings/')
	).length;
	failReload = true;
	await page.getByRole('button', { name: 'โหลดล่าสุดแล้วลองใหม่', exact: true }).click();
	await expect(page.getByText('โหลดข้อมูลล่าสุดไม่พร้อม', { exact: true })).toBeVisible();
	expect(writes).toHaveLength(1);
	failReload = false;
	await page.getByRole('button', { name: 'โหลดล่าสุดแล้วลองใหม่', exact: true }).click();
	await expect.poll(() => writes.length).toBe(2);
	const next = page.locator('[data-calendar-date="2026-10-13"]');
	await expect(next).toBeEnabled();
	await next.click();
	expect(writes).toHaveLength(2);
	releaseRetry();
	await expect(next).not.toHaveAttribute('aria-busy', 'true');
	expect(writes).toHaveLength(3);
	expect(writes[2]).toMatchObject({
		rowVersion: 7,
		days: [{ date: '2026-10-13', counted: false }]
	});
	expect(writes[1]).toMatchObject({ rowVersion: 6 });
	expect(writes[1].days).toEqual(
		expect.arrayContaining([
			expect.objectContaining({ date, counted: true }),
			expect.objectContaining({ date: '2026-10-12', counted: false })
		])
	);
	expect(
		api.reads.filter((value) => value !== '/api/attendance/days' && !value.includes('/settings/'))
			.length
	).toBe(otherReads);
	await expect(page.getByLabel('เข้าสายหลังเวลา')).toHaveValue('08:30');
	await page.getByRole('button', { name: 'บันทึกตั้งค่า', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	expect(api.writes[0].data).toMatchObject({
		rowVersion: 8,
		configuration: { lateAfter: '08:30:00' }
	});
});

for (const enteredNote of [false, true]) {
	test(`retry preserves the latest stored note unless explicitly entered=${enteredNote}`, async ({
		page
	}) => {
		await fixture(page);
		let conflicted = false;
		const writes: SaveAttendanceDays[] = [];
		await page.route(
			(url) => url.pathname === '/api/attendance/days',
			(route) =>
				reply(route, [
					{ date, counted: true, note: conflicted ? 'หมายเหตุใหม่จากอีกคน' : 'หมายเหตุเดิม' }
				])
		);
		await page.route(
			(url) => url.pathname.includes('/api/attendance/settings/'),
			(route) => reply(route, { ...settings, rowVersion: conflicted ? 4 : 1 })
		);
		await page.route(
			(url) => url.pathname.includes('/api/attendance/days/'),
			(route) => {
				writes.push(route.request().postDataJSON());
				if (writes.length === 1) {
					conflicted = true;
					return reply(route, 'ข้อมูลเปลี่ยนแล้ว', 409);
				}
				return reply(route, { ...settings, rowVersion: 5 });
			}
		);
		await page.goto(path);
		if (enteredNote)
			await page
				.getByLabel('หมายเหตุวันหยุด / กิจกรรม', { exact: true })
				.fill('หมายเหตุที่กรอกเอง');
		const cell = page.locator(`[data-calendar-date="${date}"]`);
		await expect(cell).toBeEnabled();
		await cell.click();
		await expect(page.getByText('ข้อมูลเปลี่ยนแล้ว', { exact: true })).toBeVisible();
		await page.getByRole('button', { name: 'โหลดล่าสุดแล้วลองใหม่', exact: true }).click();
		await expect(cell).not.toHaveAttribute('aria-busy', 'true');
		expect(writes).toHaveLength(2);
		expect(writes[1]).toMatchObject({
			rowVersion: 4,
			days: [
				{ date, counted: false, note: enteredNote ? 'หมายเหตุที่กรอกเอง' : 'หมายเหตุใหม่จากอีกคน' }
			]
		});
		await expect(cell).toHaveAttribute(
			'title',
			enteredNote ? 'หมายเหตุที่กรอกเอง' : 'หมายเหตุใหม่จากอีกคน'
		);
	});
}

test('discarding failed edits loads authoritative dates and never replays the queue', async ({
	page
}) => {
	await fixture(page);
	let attempts = 0;
	await page.route(
		(url) => url.pathname.includes('/api/attendance/days/'),
		(route) => {
			attempts++;
			return reply(route, 'เครือข่ายไม่พร้อม', 503);
		}
	);
	await page.goto(path);
	const cell = page.locator(`[data-calendar-date="${date}"]`);
	await cell.click();
	await expect(page.getByText('เครือข่ายไม่พร้อม', { exact: true })).toBeVisible();
	await page.route(
		(url) => url.pathname === '/api/attendance/days',
		(route) => reply(route, [{ date, counted: true, note: 'ข้อมูลจากเซิร์ฟเวอร์' }])
	);
	await page.getByRole('button', { name: 'ยกเลิกรายการค้าง', exact: true }).click();
	await expect(cell).not.toHaveAttribute('aria-busy', 'true');
	await expect(cell).toHaveAttribute('aria-pressed', 'true');
	await expect(cell).toHaveAttribute('title', 'ข้อมูลจากเซิร์ฟเวอร์');
	expect(attempts).toBe(1);
	await expect(page.getByRole('button', { name: 'เดือนถัดไป', exact: true })).toBeEnabled();
});

test('logout disposes unsent calendar edits and ignores the in-flight response', async ({
	page
}) => {
	await fixture(page);
	let release!: () => void;
	const held = new Promise<void>((resolve) => (release = resolve));
	let attempts = 0;
	await page.route(
		(url) => url.pathname.includes('/api/attendance/days/'),
		async (route) => {
			attempts++;
			await held;
			await reply(route, { ...settings, rowVersion: 2 });
		}
	);
	await page.goto(path);
	await page.locator(`[data-calendar-date="${date}"]`).click();
	await page.locator('[data-calendar-date="2026-10-12"]').click();
	await page.getByRole('banner').getByRole('button').last().click();
	await page.getByRole('menuitem', { name: 'ออกจากระบบ' }).click();
	await expect(page).toHaveURL(new URL('/', page.url()).href);
	release();
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);
	expect(attempts).toBe(1);
});

test('archived calendar exposes saved counting but never writes from its day cells', async ({
	page
}) => {
	const api = await fixture(page, true, true);
	await page.goto(path);
	const cell = page.locator(`[data-calendar-date="${date}"]`);
	await expect(cell).toHaveAttribute('aria-pressed', 'true');
	await expect(cell).toBeDisabled();
	await expect(page.locator('[data-calendar-date="2026-09-30"]')).toBeDisabled();
	await page
		.getByRole('button', { name: /ดูรายละเอียด วันหยุดโรงเรียน/ })
		.first()
		.click();
	await expect(page.getByRole('dialog')).toBeVisible();
	expect(api.writes).toHaveLength(0);
});

test('toggling a saved date preserves its note without requiring it to be entered again', async ({
	page
}) => {
	const api = await fixture(page);
	await page.route(
		(url) => url.pathname === '/api/attendance/days',
		(route) => reply(route, [{ date, counted: false, note: 'วันสอบปลายภาค' }])
	);
	await page.goto(path);
	const cell = page.locator(`[data-calendar-date="${date}"]`);
	await expect(cell).toBeEnabled();
	await expect(cell).toHaveAttribute('aria-pressed', 'false');
	await cell.click();
	await expect(cell).toHaveAttribute('aria-pressed', 'true');
	expect(api.writes[0].data).toMatchObject({
		days: [{ date, counted: true, note: 'วันสอบปลายภาค' }]
	});
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
	await expect(page.locator(`[data-calendar-date="${date}"]`)).toBeEnabled();
	const attendanceReads = api.reads.filter((value) => value.startsWith('/api/attendance')).length;
	fail = false;
	await page.getByRole('button', { name: 'ลองโหลดกิจกรรมใหม่' }).click();
	await expect(
		page.getByRole('button', { name: /ดูรายละเอียด วันหยุดโรงเรียน/ }).first()
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
	await expect(page.locator(`[data-calendar-date="${date}"]`)).toBeEnabled();
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
			const api = await fixture(page);
			await page.goto(path);
			await expect(
				page.getByRole('heading', { name: 'ปฏิทินวันประมวลผล', exact: true })
			).toBeVisible();
			await expect(
				page.getByRole('button', { name: /ดูรายละเอียด วันหยุดโรงเรียน/ }).first()
			).toBeVisible();
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			const digestSection = page.getByRole('region', { name: 'เวลาส่งสรุปครู', exact: true });
			await expect(digestSection).toBeVisible();
			const separation = await page
				.getByRole('region', { name: 'เกณฑ์และปฏิทินเช็คชื่อ', exact: true })
				.locator('form')
				.evaluate((form) => {
					const late = form.querySelector<HTMLInputElement>('#attendance-late-after')!,
						evidence = form.querySelector<HTMLInputElement>('#attendance-evidence-days')!;
					const section = form.querySelector('#attendance-digest-heading')!.closest('section')!;
					const time = form.querySelector('#attendance-digest-0')!,
						hint = form.querySelector('#attendance-digest-0-hint')!;
					const label = form.querySelector('label[for="attendance-digest-0"]')!;
					return {
						before: section.getBoundingClientRect().top - evidence.getBoundingClientRect().bottom,
						inputGap: time.getBoundingClientRect().top - label.getBoundingClientRect().bottom,
						hintGap: hint.getBoundingClientRect().top - time.getBoundingClientRect().bottom,
						topDifference: Math.abs(
							late.getBoundingClientRect().top - evidence.getBoundingClientRect().top
						)
					};
				});
			expect(separation.before).toBeGreaterThanOrEqual(16);
			expect(separation.inputGap).toBeGreaterThanOrEqual(8);
			expect(separation.hintGap).toBeGreaterThanOrEqual(7);
			if (viewport.name === 'desktop') expect(separation.topDifference).toBeLessThanOrEqual(2);
			await page
				.getByRole('region', { name: 'เกณฑ์และปฏิทินเช็คชื่อ', exact: true })
				.locator('form')
				.screenshot({
					animations: 'disabled',
					path: `/tmp/attendance-queue-form-${viewport.name}-${theme}.png`
				});
			for (let i = 1; i < 8; i++)
				await digestSection.getByRole('button', { name: 'เพิ่มเวลาสรุป', exact: true }).click();
			await expect(digestSection.locator('input')).toHaveCount(8);
			await expect(
				digestSection.getByRole('button', { name: 'เพิ่มเวลาสรุป', exact: true })
			).toBeDisabled();
			for (let i = 0; i < 8; i++)
				await page
					.getByLabel(`เวลาสรุป ${i + 1}`, { exact: true })
					.fill(`${String(8 + i).padStart(2, '0')}:00`);
			const widths = await digestSection
				.locator('input')
				.evaluateAll((inputs) => inputs.map((input) => input.getBoundingClientRect().width));
			for (const width of widths) expect(width).toBeGreaterThanOrEqual(160);
			await digestSection.screenshot({
				animations: 'disabled',
				path: `/tmp/attendance-queue-digests-${viewport.name}-${theme}.png`
			});
			await digestSection.getByRole('button', { name: 'ลบเวลาสรุป 4', exact: true }).click();
			await expect(digestSection.locator('input')).toHaveCount(7);
			await expect(page.getByLabel('เวลาสรุป 7', { exact: true })).toHaveValue('15:00');
			await page.getByRole('button', { name: 'บันทึกตั้งค่า', exact: true }).click();
			await expect.poll(() => api.writes.length).toBe(1);
			expect(api.writes[0].data).toMatchObject({
				configuration: {
					digestTimes: [
						'08:00:00',
						'09:00:00',
						'10:00:00',
						'12:00:00',
						'13:00:00',
						'14:00:00',
						'15:00:00'
					]
				}
			});
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
