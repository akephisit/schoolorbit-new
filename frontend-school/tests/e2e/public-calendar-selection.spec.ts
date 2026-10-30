import { expect, test } from '@playwright/test';
import { startPublicCalendarServer } from './fixtures/public-calendar-server';
import { year } from './fixtures/staff-home-route-data';

let fixture: Awaited<ReturnType<typeof startPublicCalendarServer>>;
test.describe.configure({ mode: 'serial', timeout: 60000 });
test.use({ serviceWorkers: 'block', hasTouch: true });
test.beforeAll(async () => {
	fixture = await startPublicCalendarServer();
});
test.afterAll(async () => {
	await fixture?.close();
});
test.beforeEach(async ({ page }) => {
	await page.addInitScript(() =>
		localStorage.setItem('ios-install-dismissed', Date.now().toString())
	);
	await page.route('https://fonts.**', (route) => route.abort());
});

for (const width of [1543, 390]) {
	test(`public selection is explicit and clears on dismissal and navigation at ${width}px`, async ({
		page
	}) => {
		await page.setViewportSize({ width, height: 884 });
		await page.goto(`${fixture.baseUrl}/calendar?month=2026-10`);
		const selectedDays = page.locator('[data-calendar-date][aria-pressed="true"]');
		const day = page.locator('[data-calendar-date="2026-10-01"]');
		await expect(day).toBeVisible();
		await expect(selectedDays).toHaveCount(0);
		await day.tap();
		await expect(day).toHaveAttribute('aria-pressed', 'true');
		const details =
			width < 1024 ? page.getByRole('dialog') : page.locator('[data-public-calendar-details]');
		await expect(details).toContainText('สอบช่วงเช้า');
		await page.keyboard.press('Escape');
		await expect(selectedDays).toHaveCount(0);
		await day.tap();
		await details
			.getByRole('button', { name: width < 1024 ? 'Close' : 'ปิดรายละเอียดกิจกรรม', exact: true })
			.click();
		await expect(selectedDays).toHaveCount(0);
		await day.tap();
		if (width < 1024)
			await page.locator('[data-slot="dialog-overlay"]').tap({ position: { x: 8, y: 8 } });
		else await page.getByRole('heading', { name: 'ปฏิทินโรงเรียน', exact: true }).tap();
		await expect(selectedDays).toHaveCount(0);
		await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
		await expect(page).toHaveURL(/month=2026-11/);
		await expect(selectedDays).toHaveCount(0);
		await page.locator('#public-calendar-year').click();
		await page.getByRole('option', { name: 'ปีการศึกษา 2569', exact: true }).click();
		await expect(page).toHaveURL(new RegExp(`academicYearId=${year}`));
		await expect(selectedDays).toHaveCount(0);
	});
}

test('standalone calendar fits changing heights, links home and keeps embed chrome compact', async ({
	page
}) => {
	await page.setViewportSize({ width: 1543, height: 700 });
	await page.goto(`${fixture.baseUrl}/calendar?month=2026-10`);
	await expect(page.locator('[data-calendar-date="2026-10-01"]')).toBeVisible();
	for (const viewport of [
		{ width: 1543, height: 700 },
		{ width: 1543, height: 1050 },
		{ width: 390, height: 844 }
	]) {
		await page.setViewportSize(viewport);
		await expect
			.poll(() =>
				page.locator('main').evaluate((element) => element.scrollHeight - element.clientHeight)
			)
			.toBeLessThanOrEqual(1);
		const bottom = (await page.locator('[data-calendar-date]').last().boundingBox())!;
		expect(bottom.y + bottom.height).toBeGreaterThan(viewport.height - 80);
		expect(bottom.y + bottom.height).toBeLessThanOrEqual(viewport.height);
	}
	const home = page.getByRole('link', { name: 'กลับหน้าแรก', exact: true });
	await expect(home).toHaveAttribute('href', '/');
	await home.click();
	await expect(page).toHaveURL(`${fixture.baseUrl}/`);
	await page.goto(`${fixture.baseUrl}/calendar/embed?month=2026-10`);
	await expect(page.locator('[data-calendar-date="2026-10-01"]')).toBeVisible();
	await expect(page.getByRole('link', { name: 'กลับหน้าแรก', exact: true })).toHaveCount(0);
	await expect(page.getByRole('heading', { name: 'ปฏิทินโรงเรียน', exact: true })).toHaveCount(0);
});
