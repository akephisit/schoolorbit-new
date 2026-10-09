import { expect, test } from '@playwright/test';
import { startPublicCalendarServer } from './fixtures/public-calendar-server';
import { year } from './fixtures/staff-home-route-data';
let fixture: Awaited<ReturnType<typeof startPublicCalendarServer>>;
test.describe.configure({ mode: 'serial' });
test.use({ serviceWorkers: 'block' });
test.beforeAll(async () => {
	fixture = await startPublicCalendarServer();
});
test.afterAll(async () => {
	await fixture?.close();
});
test.beforeEach(async ({ page }) => {
	test.setTimeout(60000);
	fixture.requests.length = 0;
	fixture.setFailure(false);
	await page.addInitScript(() =>
		localStorage.setItem('ios-install-dismissed', Date.now().toString())
	);
	await page.route('https://fonts.**', (route) => route.abort());
});
test.afterEach(() => fixture.release());

test('public SSR and month navigation use actual academic-year dates and retain the workspace', async ({
	page
}) => {
	await page.goto(`${fixture.baseUrl}/calendar?month=2026-05&academicYearId=${year}`);
	await expect(page.locator('#public-calendar-year')).toContainText('ปีการศึกษา 2569');
	await expect(page.getByRole('button', { name: 'เดือนก่อนหน้า', exact: true })).toBeDisabled();
	expect(
		fixture.requests
			.find((url) => url.pathname.endsWith('/calendar/events'))
			?.searchParams.get('from')
	).toBe('2026-05-14');
	await page.goto(`${fixture.baseUrl}/calendar?month=2026-10&academicYearId=${year}`);
	await expect(page.locator('[data-calendar-date="2026-10-01"]')).toBeVisible();
	fixture.hold();
	await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
	await expect(page).toHaveURL(/month=2026-11/);
	await expect(page.locator('[data-calendar-date="2026-11-01"]')).toBeVisible();
	await expect(page.getByRole('status', { name: /กำลังโหลดกิจกรรม/ })).toBeVisible();
	fixture.release();
	await expect(page.getByRole('status', { name: /กำลังโหลดกิจกรรม/ })).toHaveCount(0);
	await page.goto(`${fixture.baseUrl}/calendar?month=2027-04&academicYearId=${year}`);
	await expect(page.getByRole('button', { name: 'เดือนถัดไป', exact: true })).toBeDisabled();
	expect(
		fixture.requests
			.filter((url) => url.pathname.endsWith('/calendar/events'))
			.at(-1)
			?.searchParams.get('to')
	).toBe('2027-04-09');
});
for (const width of [375, 1280]) {
	test(`public partial search, result navigation and theme layout at ${width}px`, async ({
		page
	}, testInfo) => {
		await page.setViewportSize({ width, height: 884 });
		await page.goto(`${fixture.baseUrl}/calendar?month=2026-10&academicYearId=${year}`);
		await expect(page.locator('[data-calendar-date="2026-10-01"]')).toBeVisible();
		await page.getByRole('button', { name: 'ค้นหากิจกรรม', exact: true }).click();
		await page.getByLabel('คำค้นหากิจกรรม').fill('ทัศน');
		const dialog = page.getByRole('dialog');
		await expect(dialog.getByRole('button', { name: /ทัศนศึกษาพฤศจิกายน/ })).toBeVisible();
		const read = fixture.requests.find((url) => url.searchParams.get('search') === 'true');
		expect(read?.searchParams.get('q')).toBe('ทัศน');
		expect(read?.searchParams.get('from')).toBe('2026-05-14');
		expect(read?.searchParams.get('to')).toBe('2027-04-09');
		await page.screenshot({
			path: testInfo.outputPath(`search-${width}-light.png`),
			animations: 'disabled'
		});
		await page.keyboard.press('Escape');
		await expect(dialog).toHaveCount(0);
		await page.evaluate(() => document.documentElement.classList.add('dark'));
		await page.getByRole('button', { name: 'ค้นหากิจกรรม', exact: true }).click();
		await page.getByLabel('คำค้นหากิจกรรม').fill('ทัศน');
		await expect(dialog.getByRole('button', { name: /ทัศนศึกษาพฤศจิกายน/ })).toBeVisible();
		await page.screenshot({
			path: testInfo.outputPath(`search-${width}-dark.png`),
			animations: 'disabled'
		});
		await dialog.getByRole('button', { name: /ทัศนศึกษาพฤศจิกายน/ }).click();
		await expect(page).toHaveURL(/month=2026-11/);
		expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
			true
		);
	});
}
test('a failed public first read offers a focused successful retry', async ({ page }) => {
	fixture.setFailure(true);
	await page.goto(`${fixture.baseUrl}/calendar?month=2026-10`);
	await expect(page.getByText('ปฏิทินทดสอบยังไม่พร้อม')).toBeVisible();
	fixture.setFailure(false);
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.locator('[data-calendar-date="2026-10-01"]')).toBeVisible();
});

test('selecting an academic year jumps inside its actual date bounds', async ({ page }) => {
	await page.goto(`${fixture.baseUrl}/calendar?month=2026-01`);
	await expect(page.locator('#public-calendar-year')).toBeEnabled();
	await page.locator('#public-calendar-year').click();
	await page.getByRole('option', { name: 'ปีการศึกษา 2569', exact: true }).click();
	await expect(page).toHaveURL(/month=2026-05/);
	await expect(page).toHaveURL(new RegExp(`academicYearId=${year}`));
	await expect(page.getByRole('button', { name: 'เดือนก่อนหน้า', exact: true })).toBeDisabled();
	await expect
		.poll(() =>
			fixture.requests
				.filter((url) => url.pathname.endsWith('/calendar/events'))
				.at(-1)
				?.searchParams.get('from')
		)
		.toBe('2026-05-14');
});
