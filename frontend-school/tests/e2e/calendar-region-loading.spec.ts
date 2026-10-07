import { test, expect } from '@playwright/test';
import {
	mockCalendar,
	calendarPath,
	eventsPath,
	categoriesPath,
	tagsPath,
	nextYear,
	categoryId
} from './fixtures/calendar-route-data';
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.clock.setFixedTime(new Date('2026-10-01T02:00:00Z'));
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
for (const kind of ['events', 'categories', 'tags']) {
	const region = `calendar-${kind}`;
	test(`${kind} can be slow while independent calendar siblings render`, async ({ page }) => {
		const api = await mockCalendar(page, { hold: kind });
		await page.goto(calendarPath());
		await expect(page.getByTestId(region).getByRole('status')).toBeVisible();
		await expect(
			page.getByTestId(kind === 'events' ? 'calendar-categories' : 'calendar-events')
		).toContainText(kind === 'events' ? 'ทุกหมวดหมู่' : 'กิจกรรมแรก');
		for (const path of [eventsPath, categoriesPath, tagsPath]) expect(api.count(path)).toBe(1);
		expect(api.count('/api/academic/homerooms')).toBe(0);
		api.release();
		await expect(page.getByTestId(region).getByRole('status')).toHaveCount(0);
	});
	test(`${kind} has focused failure and retry`, async ({ page }) => {
		const api = await mockCalendar(page, { fail: kind });
		await page.goto(calendarPath());
		await expect(page.getByTestId(region)).toContainText(`region ${kind} ไม่พร้อม`);
		await page.getByTestId(region).getByRole('button', { name: 'ลองอีกครั้ง' }).click();
		await expect(page.getByTestId(region)).not.toContainText(`region ${kind} ไม่พร้อม`);
		for (const [other, path] of [
			['events', eventsPath],
			['categories', categoriesPath],
			['tags', tagsPath]
		])
			expect(api.count(path)).toBe(other === kind ? 2 : 1);
	});
}
test('month and committed search preserve URL history while rereading events only', async ({
	page
}) => {
	const api = await mockCalendar(page);
	await page.goto(calendarPath());
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
	await expect(page).toHaveURL(/month=2026-11/);
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมพฤศจิกายน');
	await page.getByPlaceholder('ค้นหาชื่อ รายละเอียด สถานที่ หรือแท็ก').fill('ไม่มีรายการ');
	await page.getByRole('button', { name: /^กรอง(?: \d+)?$/ }).click();
	await expect(page).toHaveURL(/q=/);
	await expect(page.getByTestId('calendar-events')).toContainText('ยังไม่มีกิจกรรม');
	await page.goBack();
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมพฤศจิกายน');
	await page.goBack();
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	expect(api.count(categoriesPath)).toBe(1);
	expect(api.count(tagsPath)).toBe(1);
	expect(api.count(eventsPath)).toBe(5);
});
test('same-context failed refresh retains events and retries only them', async ({ page }) => {
	const api = await mockCalendar(page, { fail: 'events', failAt: 2 });
	await page.goto(calendarPath());
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	await page.getByRole('button', { name: 'รีเฟรชปฏิทิน', exact: true }).click();
	await expect(page.getByTestId('calendar-events')).toContainText('region events ไม่พร้อม');
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	await page.getByTestId('calendar-events').getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	expect(api.count(categoriesPath)).toBe(1);
	expect(api.count(tagsPath)).toBe(1);
});
test('late month response cannot restore old month data', async ({ page }) => {
	const api = await mockCalendar(page, { hold: 'events' });
	await page.goto(calendarPath());
	await expect(page.getByTestId('calendar-events').getByRole('status')).toBeVisible();
	await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมพฤศจิกายน');
	api.release();
	await expect(page.getByTestId('calendar-events')).not.toContainText('กิจกรรมแรก');
	expect(api.count(categoriesPath)).toBe(1);
	expect(api.count(tagsPath)).toBe(1);
});
test('a calendar link with a different academic year reads the same date calendar', async ({
	page
}) => {
	const api = await mockCalendar(page);
	await page.goto(calendarPath(nextYear));
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	const reads = api.reads.filter((url) => url.pathname === eventsPath);
	expect(reads).toHaveLength(1);
	expect(reads[0].searchParams.has('academicYearId')).toBe(false);
	expect(reads[0].searchParams.has('academicTermId')).toBe(false);
});
test('reader does not request unopened target options', async ({ page }) => {
	const api = await mockCalendar(page, { permissions: ['calendar.read.school'] });
	await page.goto(calendarPath());
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	await expect(page.getByRole('button', { name: 'เพิ่มกิจกรรม', exact: true })).toHaveCount(0);
	expect(api.count('/api/academic/homerooms')).toBe(0);
	expect(api.count('/api/lookup/grade-levels')).toBe(0);
});
test('opened event options start together, retry locally, and retain its input draft', async ({
	page
}) => {
	const api = await mockCalendar(page, { fail: 'options' });
	await page.goto(calendarPath());
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	await page.getByRole('button', { name: 'แก้ไข กิจกรรมแรก', exact: true }).click();
	await page.getByLabel('ชื่อกิจกรรม', { exact: true }).fill('ร่างแก้ไข');
	await expect(page.getByTestId('calendar-target-options')).toContainText(
		'region options ไม่พร้อม'
	);
	await expect(page.getByRole('button', { name: 'บันทึกและเผยแพร่', exact: true })).toBeDisabled();
	await page
		.getByTestId('calendar-target-options')
		.getByRole('button', { name: 'ลองอีกครั้ง' })
		.click();
	await expect(page.getByRole('button', { name: 'บันทึกและเผยแพร่', exact: true })).toBeEnabled();
	await expect(page.getByLabel('ชื่อกิจกรรม', { exact: true })).toHaveValue('ร่างแก้ไข');
	expect(api.count(eventsPath)).toBe(1);
	expect(api.count(categoriesPath)).toBe(1);
	expect(api.count(tagsPath)).toBe(1);
	expect(api.count('/api/calendar/target-options')).toBe(2);
});
test('typed event save patches only events and never rereads catalogs', async ({ page }) => {
	const api = await mockCalendar(page);
	await page.goto(calendarPath());
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	await page.getByRole('button', { name: 'แก้ไข กิจกรรมแรก', exact: true }).click();
	await page.getByLabel('ชื่อกิจกรรม', { exact: true }).fill('กิจกรรมแก้ไข');
	await page.getByRole('button', { name: 'บันทึกและเผยแพร่', exact: true }).click();
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแก้ไข');
	for (const path of [eventsPath, categoriesPath, tagsPath]) expect(api.count(path)).toBe(1);
});
test('late closed event save cannot close or reset a reopened draft', async ({ page }) => {
	const api = await mockCalendar(page, { hold: 'mutation' });
	await page.goto(calendarPath());
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	await page.getByRole('button', { name: 'แก้ไข กิจกรรมแรก', exact: true }).click();
	await page.getByLabel('ชื่อกิจกรรม', { exact: true }).fill('กิจกรรมบันทึก');
	await page.getByRole('button', { name: 'บันทึกและเผยแพร่', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	await page.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	await page.getByRole('button', { name: 'แก้ไข กิจกรรมแรก', exact: true }).click();
	await page.getByLabel('ชื่อกิจกรรม', { exact: true }).fill('ร่างใหม่');
	api.release();
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมบันทึก');
	await expect(page.getByLabel('ชื่อกิจกรรม', { exact: true })).toHaveValue('ร่างใหม่');
	await expect(page.getByRole('dialog')).toBeVisible();
});
test('category label mutation patches catalog and event labels locally', async ({ page }) => {
	const api = await mockCalendar(page);
	await page.goto(calendarPath());
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	await page.getByRole('button', { name: 'หมวดหมู่และแท็ก', exact: true }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'หมวดแรก', exact: true }).click();
	await page.getByLabel('ชื่อหมวดหมู่', { exact: true }).fill('หมวดเปลี่ยน');
	await page
		.getByRole('dialog')
		.getByRole('button', { name: 'บันทึกหมวดหมู่', exact: true })
		.click();
	await expect(page.getByTestId('calendar-events')).toContainText('หมวดเปลี่ยน');
	for (const path of [eventsPath, categoriesPath, tagsPath]) expect(api.count(path)).toBe(1);
});
test('deleting an active category clears only its URL filter and rereads events once', async ({
	page
}) => {
	const api = await mockCalendar(page);
	await page.goto(`${calendarPath()}&categoryId=${categoryId}`);
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	await page.getByRole('button', { name: 'หมวดหมู่และแท็ก', exact: true }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'หมวดแรก', exact: true }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'ลบถาวร', exact: true }).click();
	await page.getByRole('alertdialog').getByRole('button', { name: /ลบ/ }).click();
	await expect(page).not.toHaveURL(/categoryId=/);
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	expect(api.count(eventsPath)).toBe(2);
	expect(api.count(categoriesPath)).toBe(1);
	expect(api.count(tagsPath)).toBe(1);
});

test('calendar can navigate months without an academic context switcher', async ({ page }) => {
	const api = await mockCalendar(page);
	await page.goto('/staff/calendar?month=2026-10');
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	await expect(page.getByTestId('academic-context-switcher')).toHaveCount(0);
	await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมพฤศจิกายน');
	expect(
		api.reads
			.filter((url) => url.pathname === eventsPath)
			.at(-1)
			?.searchParams.get('from')
	).toBe('2026-11-01');
});

test('renaming a tag removes events that stop matching the committed search without rereading', async ({
	page
}) => {
	const api = await mockCalendar(page);
	await page.goto(`${calendarPath()}&q=${encodeURIComponent('แท็กแรก')}`);
	await expect(page.getByTestId('calendar-events')).toContainText('กิจกรรมแรก');
	await page.getByRole('button', { name: 'หมวดหมู่และแท็ก', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('tab', { name: 'แท็ก', exact: true }).click();
	await dialog.getByRole('button', { name: 'แท็กแรก', exact: true }).click();
	await page.getByLabel('ชื่อแท็ก', { exact: true }).fill('แท็กเปลี่ยน');
	await dialog.getByRole('button', { name: 'บันทึกแท็ก', exact: true }).click();
	await expect(page.getByTestId('calendar-events')).not.toContainText('กิจกรรมแรก');
	for (const path of [eventsPath, categoriesPath, tagsPath]) expect(api.count(path)).toBe(1);
});
