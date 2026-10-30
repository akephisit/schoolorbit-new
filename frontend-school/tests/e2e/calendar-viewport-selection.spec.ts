import { expect, test } from '@playwright/test';
import {
	calendarPath,
	eventsPath,
	makeApprovedCalendarEvent,
	mockCalendar
} from './fixtures/calendar-route-data';

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
		expect(dayBox.y + dayBox.height).toBeGreaterThan(box.y + box.height - 80);
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
	await expect.poll(() => day.evaluate((element) => element.matches(':focus-visible'))).toBe(false);
	await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
	await expect(page.locator('[data-calendar-date="2026-11-01"]')).toBeVisible();
	await expect(selectedDays).toHaveCount(0);
});

test('short calendar rows keep event targets inside the day and expose every hidden activity', async ({
	page
}) => {
	const base = makeApprovedCalendarEvent({
		title: 'กิจกรรมหนาแน่น',
		startDate: '2026-10-01',
		endDate: '2026-10-01',
		allDay: true,
		startTime: null,
		endTime: null,
		isPublic: true,
		notifyAudience: false,
		reminderOffsetsDays: [],
		targets: [{ audienceType: 'all' }]
	});
	await page.route(
		(url) => url.pathname === eventsPath,
		(route) =>
			route.fulfill({
				json: {
					success: true,
					data: Array.from({ length: 8 }, (_, index) => ({
						...base,
						id: `dense-${index}`,
						title: `กิจกรรมหนาแน่น ${index + 1}`
					}))
				}
			})
	);
	await page.setViewportSize({ width: 1543, height: 700 });
	await page.goto(calendarPath());
	const day = page.locator('[data-calendar-date="2026-10-01"]');
	const entries = page.getByRole('button', { name: /^ดูรายละเอียด กิจกรรมหนาแน่น/ });
	await expect(day).toBeVisible();
	let shortCount = 0;
	for (const viewport of [
		{ width: 1543, height: 700 },
		{ width: 1543, height: 1050 },
		{ width: 390, height: 844 }
	]) {
		await page.setViewportSize(viewport);
		if (viewport.height === 1050)
			await expect.poll(() => entries.count()).toBeGreaterThan(shortCount);
		await expect
			.poll(() =>
				day.evaluate((element) => {
					const dayBox = element.getBoundingClientRect();
					const badge = Array.from(element.querySelectorAll('span')).find((span) =>
						span.textContent?.trim().startsWith('+')
					);
					if (!badge) return false;
					const badgeBox = badge.getBoundingClientRect();
					const visible = Array.from(
						element.parentElement!.querySelectorAll(
							'button[aria-label^="ดูรายละเอียด กิจกรรมหนาแน่น"]'
						)
					);
					const hidden = Number(badge.textContent?.trim().slice(1));
					return (
						visible.length > 0 &&
						visible.length + hidden === 8 &&
						visible.every((entry) => {
							const box = entry.getBoundingClientRect();
							return (
								box.y >= dayBox.y &&
								box.bottom <= badgeBox.y &&
								box.x >= dayBox.x &&
								box.right <= dayBox.right
							);
						})
					);
				})
			)
			.toBe(true);
		if (viewport.height === 700) shortCount = await entries.count();
	}
	await day.click({ position: { x: 5, y: 5 } });
	await expect(
		page.getByRole('dialog', { name: 'รายละเอียดปฏิทิน', exact: true }).getByRole('article')
	).toHaveCount(8);
});
