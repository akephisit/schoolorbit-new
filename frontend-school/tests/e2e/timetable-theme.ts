import { expect, type Page } from '@playwright/test';

export async function setTimetableTheme(page: Page, theme: 'light' | 'dark'): Promise<void> {
	await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
	const toggle = page.getByRole('button', { name: 'Toggle Dark Mode', exact: true });
	await expect(toggle).toBeVisible();
	const storedTheme = await page.evaluate(
		() => JSON.parse(localStorage.getItem('ui-preferences') ?? '{}').theme
	);
	if (storedTheme !== theme) {
		const dark = await page
			.locator('html')
			.evaluate((element) => element.classList.contains('dark'));
		if (dark === (theme === 'dark')) {
			await toggle.click();
			if (theme === 'dark') await expect(page.locator('html')).not.toHaveClass(/\bdark\b/);
			else await expect(page.locator('html')).toHaveClass(/\bdark\b/);
		}
		await toggle.click();
	}
	await assertTimetableTheme(page, theme);
}

export async function assertTimetableTheme(page: Page, theme: 'light' | 'dark'): Promise<void> {
	if (theme === 'dark') await expect(page.locator('html')).toHaveClass(/\bdark\b/);
	else await expect(page.locator('html')).not.toHaveClass(/\bdark\b/);
	await expect
		.poll(() =>
			page.evaluate(() => JSON.parse(localStorage.getItem('ui-preferences') ?? '{}').theme)
		)
		.toBe(theme);
	const tokenBrightness = await page.evaluate(() => {
		const probe = document.createElement('span');
		probe.style.color = 'var(--background)';
		document.body.append(probe);
		const context = document.createElement('canvas').getContext('2d')!;
		context.fillStyle = getComputedStyle(probe).color;
		context.fillRect(0, 0, 1, 1);
		probe.remove();
		return (
			Array.from(context.getImageData(0, 0, 1, 1).data)
				.slice(0, 3)
				.reduce((sum, channel) => sum + channel, 0) /
			(3 * 255)
		);
	});
	if (theme === 'dark') expect(tokenBrightness).toBeLessThan(0.3);
	else expect(tokenBrightness).toBeGreaterThan(0.8);
}
