import { expect, test } from '@playwright/test';
import type { SessionDto } from '../../src/lib/api/auth.js';
import { activityStart, mockSessionActivity } from './fixtures/session-activity';

test.use({ serviceWorkers: 'block', launchOptions: { args: ['--no-proxy-server'] } });

for (const width of [320, 1280]) {
	test(`login remember-me checkbox keeps a compact form at ${width}px`, async ({
		context,
		page
	}) => {
		await page.setViewportSize({ width, height: width === 320 ? 568 : 900 });
		await context.route('**/api/auth/me', (route) =>
			route.fulfill({ status: 401, json: { success: false, error: 'No fixture session' } })
		);
		await context.route('**/api/auth/restore', (route) =>
			route.fulfill({ status: 401, json: { success: false, error: 'No fixture grant' } })
		);
		await page.goto('/login');
		const remember = page.getByRole('checkbox', { name: 'จดจำฉันไว้', exact: true });
		await expect(remember).toBeVisible();
		await expect(remember).toHaveAccessibleDescription('');
		await expect(page.getByText('ใช้เฉพาะอุปกรณ์ส่วนตัว', { exact: true })).toHaveCount(0);
		await remember.click();
		await expect(remember).toBeChecked();
		for (const dark of [false, true]) {
			await page.evaluate((dark) => document.documentElement.classList.toggle('dark', dark), dark);
			expect(
				await page.evaluate(() => ({
					width: document.documentElement.scrollWidth <= innerWidth,
					height: document.documentElement.scrollHeight <= innerHeight
				}))
			).toEqual({ width: true, height: true });
			await page.screenshot({
				path: `/tmp/schoolorbit-login-remember-me-${width}-${dark ? 'dark' : 'light'}.png`,
				fullPage: true
			});
		}
	});
}

for (const width of [390, 1280])
	for (const dark of [false, true]) {
		test(`remembered-device rows retain stable identity without invented expiry: ${width}px ${dark ? 'dark' : 'light'}`, async ({
			context,
			page
		}) => {
			await page.setViewportSize({ width, height: 1000 });
			await mockSessionActivity(context, 'staff', {}, { nativeTimers: true });
			const row = (id: string, rememberMe: boolean, isCurrent: boolean): SessionDto => ({
				id,
				deviceLabel: isCurrent
					? 'อุปกรณ์ส่วนตัวนี้'
					: rememberMe
						? 'โทรศัพท์ที่จดจำไว้'
						: 'เครื่องชั่วคราว',
				rememberMe,
				isCurrent,
				createdAt: new Date(activityStart - 91 * 86400_000).toISOString(),
				lastSeenAt: new Date(activityStart).toISOString(),
				idleExpiresAt: rememberMe ? null : new Date(activityStart + 7200_000).toISOString(),
				absoluteExpiresAt: rememberMe ? null : new Date(activityStart + 43200_000).toISOString()
			});
			const current = '01900000-0000-7000-8000-000000000011';
			const dormant = '01900000-0000-7000-8000-000000000012';
			const temporary = '01900000-0000-7000-8000-000000000013';
			const removed: string[] = [];
			await context.route('**/api/auth/sessions', (route) =>
				route.fulfill({
					json: {
						success: true,
						data: {
							sessions: [
								row(current, true, true),
								row(dormant, true, false),
								row(temporary, false, false)
							]
						}
					}
				})
			);
			await context.route('**/api/auth/sessions/*', (route) => {
				expect(route.request().method()).toBe('DELETE');
				removed.push(new URL(route.request().url()).pathname.split('/').at(-1)!);
				return route.fulfill({ json: { success: true, data: {} } });
			});
			await page.goto('/account/security');
			await expect(page.getByTestId('session-list')).toBeVisible();
			if (dark) await page.evaluate(() => document.documentElement.classList.add('dark'));
			for (const id of [current, dormant]) {
				const device = page.getByTestId(`session-row-${id}`);
				await expect(
					device.getByText('จดจำอุปกรณ์นี้จนกว่าจะออกจากระบบหรือนำอุปกรณ์ออก', { exact: true })
				).toBeVisible();
				await expect(device.getByText(/^หมดอายุ/)).toHaveCount(0);
			}
			await expect(page.getByTestId(`session-row-${temporary}`).getByText(/^หมดอายุ/)).toHaveCount(
				2
			);
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await page.getByTestId('session-list').screenshot({
				path: `/tmp/schoolorbit-remembered-devices-${width}-${dark ? 'dark' : 'light'}.png`
			});
			await page
				.getByTestId(`session-row-${dormant}`)
				.getByRole('button', { name: 'นำอุปกรณ์ออก', exact: true })
				.click();
			await expect(page.getByTestId(`session-row-${dormant}`)).toHaveCount(0);
			expect(removed).toEqual([dormant]);
			await expect(page.getByTestId(`session-row-${current}`)).toBeVisible();
		});
	}
