import { expect, test } from '@playwright/test';
import { calendarPath, mockCalendar } from './fixtures/calendar-route-data';

test.use({ serviceWorkers: 'block', hasTouch: true });
test.beforeEach(async ({ page }) => {
	await page.clock.setFixedTime(new Date('2026-10-01T02:00:00Z'));
	await page.route('https://fonts.**', (route) => route.abort());
	await mockCalendar(page);
});

test('staff calendar grows with the available viewport and scrolls only on very short screens', async ({
	page
}) => {
	await page.setViewportSize({ width: 1543, height: 884 });
	await page.goto(calendarPath());
	const region = page.getByTestId('calendar-events');
	const lastDay = page.locator('[data-calendar-date]').last();
	await expect(lastDay).toBeVisible();
	let previousHeight = 0;
	for (const viewport of [
		{ width: 1543, height: 700 },
		{ width: 1280, height: 720 },
		{ width: 1543, height: 884 },
		{ width: 1543, height: 1050 },
		{ width: 390, height: 844 }
	]) {
		await page.setViewportSize(viewport);
		await expect
			.poll(() => region.evaluate((element) => element.scrollHeight - element.clientHeight))
			.toBeLessThanOrEqual(1);
		const box = (await region.boundingBox())!;
		expect(box.y + box.height).toBeGreaterThan(viewport.height - 40);
		expect(box.y + box.height).toBeLessThanOrEqual(viewport.height);
		const dayBox = (await lastDay.boundingBox())!;
		expect(dayBox.y + dayBox.height).toBeLessThanOrEqual(box.y + box.height);
		if (viewport.width === 1543) {
			expect(box.height).toBeGreaterThan(previousHeight);
			previousHeight = box.height;
		}
	}
	await page.setViewportSize({ width: 390, height: 480 });
	await lastDay.scrollIntoViewIfNeeded();
	await expect(lastDay).toBeInViewport();
	await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).scrollIntoViewIfNeeded();
	await expect(page.getByRole('button', { name: 'เดือนถัดไป', exact: true })).toBeInViewport();
});

test('staff selection exists only while its day or event details are open', async ({ page }) => {
	await page.goto(calendarPath());
	const selectedDays = page.locator('[data-calendar-date][aria-pressed="true"]');
	const day = page.locator('[data-calendar-date="2026-10-20"]');
	const popup = page.getByRole('dialog', { name: 'รายละเอียดปฏิทิน', exact: true });
	await expect(day).toBeVisible();
	await expect(selectedDays).toHaveCount(0);
	await day.click({ position: { x: 5, y: 5 } });
	await expect(day).toHaveAttribute('aria-pressed', 'true');
	await page.keyboard.press('Escape');
	await expect(popup).toHaveCount(0);
	await expect(selectedDays).toHaveCount(0);
	await expect(day).toBeFocused();
	await page.keyboard.press('Enter');
	await expect(day).toHaveAttribute('aria-pressed', 'true');
	await popup.getByRole('button', { name: 'ปิดรายละเอียดกิจกรรม' }).click();
	await expect(selectedDays).toHaveCount(0);
	await day.click({ position: { x: 5, y: 5 } });
	await page.locator('[data-calendar-date="2026-10-21"]').tap({ position: { x: 5, y: 5 } });
	await expect(popup).toHaveCount(0);
	await expect(selectedDays).toHaveCount(0);
	await expect(day).not.toBeFocused();
	await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
	await expect(page.locator('[data-calendar-date="2026-11-01"]')).toBeVisible();
	await expect(selectedDays).toHaveCount(0);
});
