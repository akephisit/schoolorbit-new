import { devices, expect, test } from '@playwright/test';
import { makeApprovedCalendarEvent } from './fixtures/calendar-route-data';
import { year } from './fixtures/staff-home-route-data';
import { startPublicCalendarServer } from './fixtures/public-calendar-server';
let fixture: Awaited<ReturnType<typeof startPublicCalendarServer>>;
test.describe.configure({ timeout: 60000 });
test.beforeAll(async () => {
	fixture = await startPublicCalendarServer();
});
test.afterAll(async () => {
	await fixture?.close();
});

test.use({ ...devices['iPhone 13'], defaultBrowserType: 'chromium', serviceWorkers: 'block' });

test('closes the embedded calendar day dialog when its overlay is tapped', async ({ page }) => {
	await page.addInitScript(() => {
		localStorage.setItem('ios-install-dismissed', Date.now().toString());
	});
	await page.route('**/api/public/academic-context/options', (route) =>
		route.fulfill({
			json: {
				success: true,
				data: {
					years: [
						{
							id: year,
							name: '2569',
							year: 2569,
							status: 'active',
							startDate: '2026-05-01',
							endDate: '2027-04-09',
							closedOn: null
						}
					],
					terms: [],
					activeAcademicYearId: year,
					activeAcademicTermId: null
				}
			}
		})
	);
	await page.route('https://fonts.**', (route) => route.abort());
	await page.route('**/api/public/calendar/events?*', async (route) => {
		await route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify({ success: true, data: [] })
		});
	});

	await page.goto(`${fixture.baseUrl}/calendar/embed?month=2026-10&academicYearId=${year}`);
	await expect(page.locator('html')).toHaveAttribute('data-schoolorbit-app-mounted', 'true');
	await page.locator('[data-calendar-date="2026-10-01"]').tap();

	const dialog = page.getByRole('dialog');
	await expect(dialog).toBeVisible();

	await page.locator('[data-slot="dialog-overlay"]').tap({ position: { x: 8, y: 8 } });

	await expect(dialog).toBeHidden();
});

for (const width of [375, 620]) {
	test(`public mobile calendar gives timed activities the full label at ${width}px`, async ({
		page
	}, testInfo) => {
		await page.setViewportSize({ width, height: 884 });
		await page.clock.setFixedTime(new Date('2026-10-01T02:00:00Z'));
		await page.route('https://fonts.**', (route) => route.abort());
		const event = makeApprovedCalendarEvent({
			title: 'สอบช่วงเช้า',
			startDate: '2026-10-01',
			endDate: '2026-10-01',
			allDay: false,
			startTime: '08:30',
			endTime: '09:30',
			isPublic: true,
			notifyAudience: false,
			reminderOffsetsDays: [],
			targets: [{ audienceType: 'all' }]
		});
		await page.route('**/api/public/calendar/events?*', (route) =>
			route.fulfill({
				json: { success: true, data: [event] }
			})
		);
		await page.goto(`${fixture.baseUrl}/calendar?month=2026-10`);
		const entry = page.locator('[title="08:30 สอบช่วงเช้า"]');
		await expect(entry).toBeVisible();
		expect(await entry.innerText()).toBe('สอบช่วงเช้า');
		await page.screenshot({ path: testInfo.outputPath(`public-calendar-${width}.png`) });
		await page.locator('[data-calendar-date="2026-10-01"]').tap();
		await expect(page.getByRole('dialog')).toContainText('สอบช่วงเช้า');
		await expect(page.getByRole('dialog')).toContainText('08:30 – 09:30');
		await page.keyboard.press('Escape');
		await expect(page.getByRole('dialog')).toHaveCount(0);
		await page.setViewportSize({ width: 1280, height: 884 });
		expect(await entry.innerText()).toBe('08:30 สอบช่วงเช้า');
	});
}
