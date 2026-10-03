import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer as createHttpServer, type Server } from 'node:http';
import { createServer, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
let devServer: ViteDevServer;
let apiServer: Server;
let baseUrl: string;
let brandingMode: 'available' | 'absent' | 'broken' | 'unavailable' = 'available';
let brandingGate: Promise<void> | undefined;
let releaseBranding: (() => void) | undefined;
let loginGate: Promise<void> | undefined;
let releaseLogin: (() => void) | undefined;
const brandingRequests: { cookie?: string; origin?: string }[] = [];
test.describe.configure({ mode: 'serial' });
test.use({ serviceWorkers: 'block' });
test.beforeAll(async () => {
	apiServer = createHttpServer(async (req, res) => {
		const endpoint = new URL(req.url ?? '/', 'http://test').pathname;
		res.setHeader('access-control-allow-origin', req.headers.origin ?? '*');
		res.setHeader('access-control-allow-credentials', 'true');
		res.setHeader('access-control-allow-headers', 'Content-Type,X-School-Subdomain');
		res.setHeader('content-type', 'application/json');
		if (req.method === 'OPTIONS') {
			res.writeHead(204).end();
			return;
		}
		if (endpoint === '/deployment-status') {
			res.end(JSON.stringify({ status: 'ready', releaseId: 'fixture' }));
			return;
		}
		if (endpoint === '/api/school/public') {
			brandingRequests.push({ cookie: req.headers.cookie, origin: req.headers.origin });
			if (brandingGate) await brandingGate;
			if (brandingMode === 'unavailable') {
				res
					.writeHead(503)
					.end(JSON.stringify({ success: false, error: 'Fixture branding unavailable' }));
				return;
			}
			res.end(
				JSON.stringify({
					success: true,
					data: {
						schoolName: 'โรงเรียนสาธิตทดสอบ',
						logoFileId: brandingMode === 'absent' ? null : '11111111-1111-4111-8111-111111111111'
					}
				})
			);
			return;
		}
		if (endpoint.startsWith('/api/public/files/')) {
			if (brandingMode === 'broken') {
				res.writeHead(404).end();
				return;
			}
			res.setHeader('content-type', 'image/svg+xml');
			res.end(
				'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 104"><path fill="#2563eb" d="M40 3 75 23V60Q70 90 40 101 10 90 5 60V23Z"/></svg>'
			);
			return;
		}
		if (endpoint === '/api/auth/login' && loginGate) await loginGate;
		res
			.writeHead(401)
			.end(JSON.stringify({ success: false, error: 'ข้อมูลเข้าสู่ระบบไม่ถูกต้อง' }));
	});
	await new Promise<void>((done) => apiServer.listen(0, '127.0.0.1', done));
	const apiAddress = apiServer.address();
	if (!apiAddress || typeof apiAddress === 'string') throw new Error('API fixture did not start');
	process.env.PUBLIC_BACKEND_URL = `http://127.0.0.1:${apiAddress.port}`;
	process.env.PUBLIC_VAPID_KEY = 'test';
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
test.beforeEach(() => {
	brandingMode = 'available';
	brandingRequests.length = 0;
	brandingGate = undefined;
	releaseBranding = undefined;
	loginGate = undefined;
	releaseLogin = undefined;
});
test.afterEach(() => {
	releaseBranding?.();
	releaseLogin?.();
});
test.afterAll(async () => {
	await devServer?.close();
	await new Promise<void>((done, reject) =>
		apiServer.close((error) => (error ? reject(error) : done()))
	);
});

for (const viewport of [
	{ width: 320, height: 568 },
	{ width: 375, height: 667 },
	{ width: 390, height: 844 },
	{ width: 1280, height: 900 }
]) {
	test(`login fits ${viewport.width}x${viewport.height} and uses the school crest without a frame`, async ({
		page
	}) => {
		await page.setViewportSize(viewport);
		await page.goto(`${baseUrl}/login`);
		await expect(page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true })).toBeVisible();
		await expect(page.getByText('โรงเรียนสาธิตทดสอบ', { exact: true })).toBeVisible();
		await expect(
			page.getByTestId('login-school-crest').getByRole('img', { name: 'ตราโรงเรียน' })
		).toBeVisible();
		const geometry = await page.evaluate(() => ({
			height: document.documentElement.scrollHeight,
			width: document.documentElement.scrollWidth,
			viewportHeight: innerHeight,
			viewportWidth: innerWidth,
			crestBorder: getComputedStyle(document.querySelector('[data-testid="login-school-crest"]')!)
				.borderTopWidth,
			crestBackground: getComputedStyle(
				document.querySelector('[data-testid="login-school-crest"]')!
			).backgroundColor
		}));
		expect(geometry.height).toBeLessThanOrEqual(geometry.viewportHeight);
		expect(geometry.width).toBeLessThanOrEqual(geometry.viewportWidth);
		expect(geometry.crestBorder).toBe('0px');
		expect(geometry.crestBackground).toBe('rgba(0, 0, 0, 0)');
		expect(brandingRequests).toEqual([{ origin: baseUrl, cookie: undefined }]);
		await page.screenshot({
			path: `/tmp/schoolorbit-login-${viewport.width}-light.png`,
			fullPage: true
		});
		await page.evaluate(() => document.documentElement.classList.add('dark'));
		await expect
			.poll(() =>
				page
					.getByRole('link', { name: 'กลับหน้าหลัก' })
					.evaluate((node) => getComputedStyle(node).color)
			)
			.toBe(await page.locator('h1').evaluate((node) => getComputedStyle(node).color));
		await page.screenshot({
			path: `/tmp/schoolorbit-login-${viewport.width}-dark.png`,
			fullPage: true
		});
	});
}

test('a short viewport can scroll to every form control and return to the top', async ({
	page
}) => {
	await page.setViewportSize({ width: 375, height: 320 });
	await page.goto(`${baseUrl}/login`);
	await expect(page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true })).toBeVisible();
	await page.getByLabel('ชื่อผู้ใช้งาน (Username)').fill('fixture-teacher');
	await page.getByLabel('รหัสผ่าน').fill('fixture-only-password');
	await page.getByLabel('จดจำฉันไว้').click();
	await expect(page.getByLabel('จดจำฉันไว้')).toBeChecked();
	await page.getByRole('link', { name: 'กลับหน้าหลัก' }).scrollIntoViewIfNeeded();
	const top = await page.getByRole('link', { name: 'กลับหน้าหลัก' }).boundingBox();
	expect(top?.y).toBeGreaterThanOrEqual(0);
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('slow optional branding does not block the login form', async ({ page }) => {
	brandingGate = new Promise<void>((done) => (releaseBranding = done));
	await page.goto(`${baseUrl}/login`, { waitUntil: 'commit' });
	await expect(page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true })).toBeVisible();
	await page.getByLabel('ชื่อผู้ใช้งาน (Username)').fill('fixture-teacher');
	await page.screenshot({ path: '/tmp/schoolorbit-login-branding-pending.png', fullPage: true });
	releaseBranding?.();
	await expect(page.getByText('โรงเรียนสาธิตทดสอบ', { exact: true })).toBeVisible();
	await expect(page.getByLabel('ชื่อผู้ใช้งาน (Username)')).toHaveValue('fixture-teacher');
});

for (const mode of ['absent', 'broken', 'unavailable'] as const) {
	test(`the form remains usable when school branding is ${mode}`, async ({ page }) => {
		brandingMode = mode;
		await page.goto(`${baseUrl}/login`);
		await expect(page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true })).toBeEnabled();
		await expect(page.getByTestId('login-school-crest').locator('img')).toHaveCount(0);
		await page.getByLabel('ชื่อผู้ใช้งาน (Username)').fill('fixture-teacher');
		await expect(page.getByLabel('ชื่อผู้ใช้งาน (Username)')).toHaveValue('fixture-teacher');
	});
}

test('pending and rejected login preserve the entered fields and re-enable submission', async ({
	page
}) => {
	loginGate = new Promise<void>((done) => (releaseLogin = done));
	await page.setViewportSize({ width: 375, height: 667 });
	await page.goto(`${baseUrl}/login`);
	await page.getByLabel('ชื่อผู้ใช้งาน (Username)').fill('fixture-teacher');
	await page.getByLabel('รหัสผ่าน').fill('fixture-only-password');
	await page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true }).click();
	await expect(page.getByRole('button', { name: 'กำลังเข้าสู่ระบบ...' })).toBeDisabled();
	await page.screenshot({ path: '/tmp/schoolorbit-login-submit-pending.png', fullPage: true });
	releaseLogin?.();
	await expect(page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true })).toBeEnabled();
	await expect(page.getByLabel('ชื่อผู้ใช้งาน (Username)')).toHaveValue('fixture-teacher');
	await expect(page.getByLabel('รหัสผ่าน')).toHaveValue('fixture-only-password');
	await expect(page.getByText('ข้อมูลเข้าสู่ระบบไม่ถูกต้อง').first()).toBeVisible();
	await page.screenshot({ path: '/tmp/schoolorbit-login-error.png', fullPage: true });
});
