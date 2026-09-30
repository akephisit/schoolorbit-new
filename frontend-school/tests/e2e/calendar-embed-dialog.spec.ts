import { devices, expect, test } from '@playwright/test';
import { year } from './fixtures/staff-home-route-data';

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
							status: 'open',
							startDate: '2026-05-01',
							plannedEndDate: '2027-04-30',
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

	await page.goto(`/calendar/embed?academicYearId=${year}`);
	await page.locator('button[aria-label*="กิจกรรม"]').first().tap();

	const dialog = page.getByRole('dialog');
	await expect(dialog).toBeVisible();

	await page.locator('[data-slot="dialog-overlay"]').tap({ position: { x: 8, y: 8 } });

	await expect(dialog).toBeHidden();
});
