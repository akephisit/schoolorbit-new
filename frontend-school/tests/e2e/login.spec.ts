import { expect, test } from '@playwright/test';

const username = process.env.E2E_USERNAME || process.env.SMOKE_USERNAME;
const password = process.env.E2E_PASSWORD || process.env.SMOKE_PASSWORD;

test.describe('sandbox login', () => {
	test.skip(
		!username || !password,
		'Set E2E_USERNAME/E2E_PASSWORD or SMOKE_USERNAME/SMOKE_PASSWORD to run login E2E.'
	);

	test('logs in and reaches an authenticated app route', async ({ page, context, baseURL }) => {
		await page.goto('/login');

		await expect(page.getByRole('heading', { name: 'เข้าสู่ระบบ' })).toBeVisible();
		await page.getByLabel('ชื่อผู้ใช้งาน (Username)').fill(username ?? '');
		await page.getByLabel('รหัสผ่าน').fill(password ?? '');

		await Promise.all([
			page.waitForURL(/\/(staff|student|parent)\/?(?:[?#].*)?$/, { timeout: 15_000 }),
			page.getByRole('button', { name: 'เข้าสู่ระบบ' }).click()
		]);

		await expect(page).toHaveURL(/\/(staff|student|parent)\/?(?:[?#].*)?$/);

		const cookies = await context.cookies(baseURL ?? undefined);
		expect(cookies.some((cookie) => cookie.name === 'auth_token')).toBe(true);
	});
});
