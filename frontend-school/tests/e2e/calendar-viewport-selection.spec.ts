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

test('staff calendar grows on desktop and places the agenda below the calendar on phones', async ({
	page
}) => {
	await page.setViewportSize({ width: 1543, height: 884 });
	await page.goto(calendarPath());
	const region = page.getByTestId('calendar-events');
	const agenda = page.getByRole('region', { name: 'รายละเอียดปฏิทิน', exact: true });
	const days = region.locator('[data-calendar-date]');
	const lastDay = days.last();
	await expect(days).toHaveCount(42);
	await expect(lastDay).toBeVisible();
	await expect(agenda).toBeVisible();
	let previousDesktopGridHeight = 0;
	for (const viewport of [
		{ width: 1543, height: 700 },
		{ width: 1280, height: 720 },
		{ width: 1543, height: 884 },
		{ width: 1543, height: 1050 },
		{ width: 390, height: 844 }
	]) {
		await page.setViewportSize(viewport);
		await expect
			.poll(() =>
				region.evaluate((element) =>
					Math.max(
						element.scrollWidth - element.clientWidth,
						document.documentElement.scrollWidth - innerWidth
					)
				)
			)
			.toBeLessThanOrEqual(1);
		await expect
			.poll(() => agenda.evaluate((element) => element.scrollWidth - element.clientWidth))
			.toBeLessThanOrEqual(1);
		const regionBox = (await region.boundingBox())!;
		const firstDayBox = (await days.first().boundingBox())!;
		const lastDayBox = (await lastDay.boundingBox())!;
		const agendaBox = (await agenda.boundingBox())!;
		const gridRight = lastDayBox.x + lastDayBox.width;
		const gridBottom = lastDayBox.y + lastDayBox.height;
		expect(firstDayBox.x).toBeGreaterThanOrEqual(0);
		expect(gridRight).toBeLessThanOrEqual(viewport.width);
		expect(agendaBox.x).toBeGreaterThanOrEqual(0);
		expect(agendaBox.x + agendaBox.width).toBeLessThanOrEqual(viewport.width);
		if (viewport.width < 640) {
			expect(agendaBox.y).toBeGreaterThanOrEqual(gridBottom);
			expect(firstDayBox.height).toBeGreaterThanOrEqual(42);
		} else {
			expect(regionBox.y + regionBox.height).toBeGreaterThan(viewport.height - 40);
			expect(regionBox.y + regionBox.height).toBeLessThanOrEqual(viewport.height + 1);
			expect(agendaBox.x).toBeGreaterThan(gridRight);
			expect(agendaBox.y).toBeLessThan(firstDayBox.y + firstDayBox.height);
			await lastDay.scrollIntoViewIfNeeded();
			await expect(lastDay).toBeInViewport();
			await agenda.getByRole('heading', { level: 2 }).scrollIntoViewIfNeeded();
		}
		if (viewport.width === 1543) {
			const gridHeight = gridBottom - firstDayBox.y;
			expect(gridHeight).toBeGreaterThan(previousDesktopGridHeight);
			previousDesktopGridHeight = gridHeight;
		}
	}
	await page.setViewportSize({ width: 390, height: 480 });
	await lastDay.scrollIntoViewIfNeeded();
	await expect(lastDay).toBeInViewport();
	const agendaHeading = agenda.getByRole('heading', { name: '1 ต.ค. 2569', exact: true });
	await agendaHeading.scrollIntoViewIfNeeded();
	await expect(agendaHeading).toBeInViewport();
	await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).scrollIntoViewIfNeeded();
	await expect(page.getByRole('button', { name: 'เดือนถัดไป', exact: true })).toBeInViewport();
});

test('staff selection persists and switches with a single click, tap, or keyboard activation', async ({
	page
}) => {
	await page.setViewportSize({ width: 1280, height: 900 });
	await page.goto(calendarPath());
	const selectedDays = page.locator('[data-calendar-date][aria-pressed="true"]');
	const day = page.locator('[data-calendar-date="2026-10-20"]');
	const nextDay = page.locator('[data-calendar-date="2026-10-21"]');
	const keyboardDay = page.locator('[data-calendar-date="2026-10-22"]');
	const agenda = page.getByRole('region', { name: 'รายละเอียดปฏิทิน', exact: true });
	await expect(day).toBeVisible();
	await expect(selectedDays).toHaveCount(1);
	await expect(selectedDays).toHaveAttribute('data-calendar-date', '2026-10-01');
	await day.click({ position: { x: 5, y: 5 } });
	await expect(day).toHaveAttribute('aria-pressed', 'true');
	await expect(agenda.getByRole('heading', { name: '20 ต.ค. 2569', exact: true })).toBeVisible();
	await page.getByRole('heading', { name: 'ปฏิทินโรงเรียน', exact: true }).click();
	await expect(day).toHaveAttribute('aria-pressed', 'true');
	await expect(agenda.getByRole('heading', { name: '20 ต.ค. 2569', exact: true })).toBeVisible();
	await nextDay.tap({ position: { x: 5, y: 5 } });
	await expect(selectedDays).toHaveCount(1);
	await expect(nextDay).toHaveAttribute('aria-pressed', 'true');
	await expect(agenda.getByRole('heading', { name: '21 ต.ค. 2569', exact: true })).toBeVisible();
	await expect.poll(() => day.evaluate((element) => element.matches(':focus-visible'))).toBe(false);
	await nextDay.focus();
	await page.keyboard.press('Tab');
	await expect(keyboardDay).toBeFocused();
	await page.keyboard.press('Enter');
	await expect(keyboardDay).toHaveAttribute('aria-pressed', 'true');
	await expect(agenda.getByRole('heading', { name: '22 ต.ค. 2569', exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
	await expect(page.locator('[data-calendar-date="2026-11-01"]')).toBeVisible();
	await expect(selectedDays).toHaveCount(1);
	await expect(selectedDays).toHaveAttribute('data-calendar-date', '2026-11-01');
	await expect(agenda.getByRole('heading', { name: '1 พ.ย. 2569', exact: true })).toBeVisible();
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
	const agenda = page.getByRole('region', { name: 'รายละเอียดปฏิทิน', exact: true });
	await expect(day).toBeVisible();
	let shortCount = 0;
	for (const viewport of [
		{ width: 1543, height: 700 },
		{ width: 1543, height: 1050 }
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
	await expect(agenda.getByRole('article')).toHaveCount(8);
	await page.setViewportSize({ width: 390, height: 844 });
	await expect(day.locator('[data-calendar-day-marker="event"]')).toBeVisible();
	await expect(day.getByText(/^\+\d+$/)).toBeHidden();
	await expect(entries).toHaveCount(0);
	await day.tap();
	await expect(agenda.getByRole('article')).toHaveCount(8);
	await expect(
		agenda.getByRole('heading', { name: 'กิจกรรมหนาแน่น 8', exact: true })
	).toBeVisible();
});
