import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type ViteDevServer } from 'vite';

const adminRoot = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	'../../../frontend-admin'
);
const previousDirectory = process.cwd();
let server: ViteDevServer;
let baseUrl: string;

test.describe.configure({ mode: 'serial' });
test.use({ ignoreHTTPSErrors: true, serviceWorkers: 'block' });

test.beforeAll(async () => {
	process.chdir(adminRoot);
	process.env.PUBLIC_API_URL = 'http://127.0.0.1:9';
	server = await createServer({
		root: adminRoot,
		logLevel: 'silent',
		server: { host: '127.0.0.1', port: 0, hmr: false }
	});
	await server.listen();
	const address = server.httpServer?.address();
	if (!address || typeof address === 'string') throw new Error('Admin fixture did not start');
	baseUrl = `https://127.0.0.1:${address.port}`;
});

test.afterAll(async () => {
	await server?.close();
	process.chdir(previousDirectory);
});

for (const width of [375, 1280]) {
	test(`central introduction and login work at ${width}px`, async ({ page }) => {
		await page.route('http://127.0.0.1:9/**', (route) =>
			route.fulfill({ status: 401, json: { success: false, error: 'No fixture session' } })
		);
		await page.setViewportSize({ width, height: 900 });
		const initialized = page.waitForResponse(/\/api\/v1\/auth\/me$/);
		await page.goto(baseUrl);
		await initialized;
		await page.evaluate(() => new Promise<void>((done) => requestAnimationFrame(() => done())));
		await expect(page.getByRole('heading', { level: 1 })).toContainText('ทุกเรื่องของโรงเรียน');
		await expect(page.getByRole('link', { name: 'สำรวจบริการ' })).toBeVisible();
		await expect(page.getByRole('heading', { name: 'ดูแลระบบจากศูนย์กลาง' })).toBeVisible();
		if (width < 768) {
			await page.getByRole('button', { name: 'เปิดเมนู' }).click();
			await expect(page.getByRole('button', { name: 'ปิดเมนู' })).toHaveAttribute(
				'aria-expanded',
				'true'
			);
			await page
				.getByRole('navigation', { name: 'เมนูหลัก' })
				.getByRole('link', { name: 'รู้จัก SchoolOrbit' })
				.click();
			await expect(page.getByRole('button', { name: 'เปิดเมนู' })).toHaveAttribute(
				'aria-expanded',
				'false'
			);
		}
		expect(
			await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
		).toBe(true);
		await page.evaluate(() => window.scrollTo({ top: 0, behavior: 'instant' }));
		await page.screenshot({ path: `/tmp/schoolorbit-admin-${width}.png`, fullPage: true });
		await page.getByRole('link', { name: 'เข้าสู่ระบบ', exact: true }).first().click();
		await expect(page).toHaveURL(/\/login$/);
	});
}

test('authenticated administrators retain the existing dashboard entry', async ({ page }) => {
	await page.route('http://127.0.0.1:9/**', (route) =>
		route.fulfill({
			json: {
				success: true,
				data: { user: { id: 'fixture-admin', nationalId: '', name: 'ผู้ดูแลทดสอบ', role: 'admin' } }
			}
		})
	);
	await page.goto(baseUrl);
	await expect(page).toHaveURL(/\/dashboard$/);
	await expect(page.getByRole('heading', { name: 'ข้อมูลผู้ใช้' })).toBeVisible();
	await expect(page.getByRole('link', { name: /จัดการโรงเรียน/ }).first()).toBeVisible();
});
