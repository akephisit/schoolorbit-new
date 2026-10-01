import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
let devServer: ViteDevServer;
let baseUrl: string;

test.beforeAll(async () => {
	devServer = await createServer({
		root: frontendRoot,
		logLevel: 'silent',
		server: { host: '127.0.0.1', port: 0 }
	});
	await devServer.listen();
	const address = devServer.httpServer?.address();
	if (!address || typeof address === 'string') throw new Error('Vite test server did not start');
	baseUrl = `http://127.0.0.1:${address.port}`;
});

test.afterAll(async () => {
	await devServer.close();
});

test('visitors can discover public services and continue to login', async ({ page }) => {
	await page.goto(baseUrl);
	await page.getByRole('link', { name: 'สำรวจบริการ' }).click();
	await expect(page).toHaveURL(/#services$/);
	await expect(
		page.getByRole('heading', { name: 'เรื่องของโรงเรียน เริ่มต้นได้ที่นี่' })
	).toBeVisible();
	await expect(page.getByRole('link', { name: /ปฏิทินโรงเรียน.*ดูปฏิทิน/ })).toHaveAttribute(
		'href',
		'/calendar'
	);
	await expect(page.getByRole('link', { name: /รับสมัครนักเรียน.*ดูการรับสมัคร/ })).toHaveAttribute(
		'href',
		'/apply'
	);
	await expect(
		page.getByRole('link', { name: /ตรวจสอบเกียรติบัตร.*ตรวจสอบเอกสาร/ })
	).toHaveAttribute('href', '/verify/certificate');
	await page.getByRole('link', { name: 'เข้าสู่ระบบ', exact: true }).first().click();
	await expect(page).toHaveURL(/\/login$/);
});

test('mobile navigation is usable without horizontal overflow', async ({ page }) => {
	await page.setViewportSize({ width: 375, height: 812 });
	await page.goto(baseUrl);
	await expect(page.locator('html')).toHaveAttribute('data-schoolorbit-app-mounted', 'true');
	const menu = page.getByRole('button', { name: 'เปิดเมนู' });
	await expect(menu).toHaveAttribute('aria-expanded', 'false');
	await menu.click();
	await expect(page.getByRole('button', { name: 'ปิดเมนู' })).toHaveAttribute(
		'aria-expanded',
		'true'
	);
	await page
		.getByRole('navigation', { name: 'เมนูหลัก' })
		.getByRole('link', { name: 'บริการ' })
		.click();
	await expect(page).toHaveURL(/#services$/);
	await expect(menu).toHaveAttribute('aria-expanded', 'false');
	await expect(
		page.getByRole('heading', { name: 'เรื่องของโรงเรียน เริ่มต้นได้ที่นี่' })
	).toBeVisible();
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
		true
	);
});
