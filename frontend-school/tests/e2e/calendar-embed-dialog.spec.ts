import { devices, expect, test } from '@playwright/test';

test.use({ ...devices['iPhone 13'], defaultBrowserType: 'chromium' });

test('closes the embedded calendar day dialog when its overlay is tapped', async ({ page }) => {
	await page.route('**/api/public/calendar/events?*', async (route) => {
		await route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify({ success: true, data: [] })
		});
	});

	await page.goto('/calendar/embed');
	await page.locator('button[aria-label*="กิจกรรม"]').first().tap();

	const dialog = page.getByRole('dialog');
	await expect(dialog).toBeVisible();

	await page.locator('[data-slot="dialog-overlay"]').tap({ position: { x: 8, y: 8 } });

	await expect(dialog).toBeHidden();
});
