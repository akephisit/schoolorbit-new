import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import type { ViteDevServer } from 'vite';
import { createRequire } from 'node:module';
import { createServer as createHttpServer, type Server } from 'node:http';

const adminRoot = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	'../../../frontend-admin'
);
const previousDirectory = process.cwd();
let server: ViteDevServer;
let apiServer: Server;
let baseUrl: string;
let authenticated = false;
let unavailable = false;
let invalid = false;
let redirected = false;
const requests: { path: string; cookie?: string }[] = [];
const schoolName = 'โรงเรียนทดสอบ </script><script>alert(1)</script>';

test.describe.configure({ mode: 'serial' });
test.use({ ignoreHTTPSErrors: true, serviceWorkers: 'block' });

test.beforeAll(async () => {
	apiServer = createHttpServer((req, res) => {
		const url = new URL(req.url ?? '/', 'http://localhost');
		requests.push({ path: url.pathname + url.search, cookie: req.headers.cookie });
		res.setHeader('access-control-allow-origin', req.headers.origin ?? '*');
		res.setHeader('access-control-allow-credentials', 'true');
		res.setHeader('access-control-allow-headers', 'Content-Type');
		res.setHeader('content-type', 'application/json');
		if (req.method === 'OPTIONS') {
			res.writeHead(204).end();
			return;
		}
		if (url.pathname === '/deployment-status') {
			res.end(JSON.stringify({ status: 'ready' }));
			return;
		}
		if (url.pathname === '/api/v1/public/schools') {
			if (redirected) {
				res.writeHead(302, { location: '/redirected-directory' }).end();
				return;
			}
			if (unavailable) {
				res.writeHead(503).end('{}');
				return;
			}
			const schools = url.searchParams.has('after')
				? [{ name: 'โรงเรียนถัดไป', subdomain: 'gamma' }]
				: [
						{ name: schoolName, subdomain: invalid ? 'evil.example' : 'alpha' },
						{ name: 'โรงเรียนเบตา', subdomain: 'beta' }
					];
			res.end(
				JSON.stringify({
					success: true,
					data: { schools, nextCursor: url.searchParams.has('after') ? null : 'beta' }
				})
			);
			return;
		}
		if (url.pathname === '/api/v1/auth/me' && authenticated) {
			res.end(
				JSON.stringify({
					success: true,
					data: {
						user: {
							id: 'fixture-operator',
							email: 'operator@example.test',
							name: 'ผู้ดูแลทดสอบ',
							role: 'operator'
						},
						csrfToken: 'fixture-csrf'
					}
				})
			);
			return;
		}
		res.writeHead(401).end(JSON.stringify({ success: false, error: 'No fixture session' }));
	});
	await new Promise<void>((done) => apiServer.listen(0, '127.0.0.1', done));
	const apiAddress = apiServer.address();
	if (!apiAddress || typeof apiAddress === 'string') throw new Error('API fixture did not start');
	process.chdir(adminRoot);
	process.env.PUBLIC_API_URL = `http://127.0.0.1:${apiAddress.port}`;
	const adminRequire = createRequire(path.join(adminRoot, 'package.json'));
	const { createServer } = (await import(
		pathToFileURL(adminRequire.resolve('vite')).href
	)) as typeof import('vite');
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

test.beforeEach(() => {
	authenticated = false;
	unavailable = false;
	invalid = false;
	redirected = false;
	requests.length = 0;
});
test.afterAll(async () => {
	await server?.close();
	await new Promise<void>((done, reject) =>
		apiServer.close((error) => (error ? reject(error) : done()))
	);
	process.chdir(previousDirectory);
});

for (const width of [375, 1280]) {
	test(`central homepage, school discovery and login work at ${width}px`, async ({ page }) => {
		await page.setViewportSize({ width, height: 900 });
		const errors: string[] = [];
		page.on('pageerror', (error) => errors.push(error.message));
		const initialized = page.waitForResponse(/\/api\/v1\/auth\/me$/);
		await page.goto(baseUrl);
		await initialized;
		await page.evaluate(() => new Promise<void>((done) => requestAnimationFrame(() => done())));
		await expect(page.getByRole('heading', { level: 1 })).toContainText('ทุกเรื่องของโรงเรียน');
		await expect(page.getByRole('link', { name: 'สำรวจบริการ' })).toBeVisible();
		if (width < 768) {
			await page.getByRole('button', { name: 'เปิดเมนู' }).click();
		}
		await page
			.getByRole('navigation', { name: 'เมนูหลัก' })
			.getByRole('link', { name: 'เว็บไซต์โรงเรียน', exact: true })
			.click();
		await expect(page).toHaveURL(/\/schools$/);
		await expect(page.getByRole('heading', { level: 1, name: 'เว็บไซต์โรงเรียน' })).toBeVisible();
		await expect(page.getByRole('link', { name: new RegExp('โรงเรียนทดสอบ') })).toHaveAttribute(
			'href',
			'https://alpha.127.0.0.1/'
		);
		await page.getByRole('link', { name: 'โรงเรียนถัดไป', exact: true }).click();
		await expect(page).toHaveURL(/\/schools\?after=beta$/);
		await expect(page.getByRole('link', { name: /โรงเรียนถัดไป gamma/ })).toBeVisible();
		expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
			true
		);
		expect(errors).toEqual([]);
		await page.screenshot({
			path: `/tmp/schoolorbit-public-directory-${width}.png`,
			fullPage: true
		});
		await page.getByRole('link', { name: 'เข้าสู่ระบบบัญชีบริการ', exact: true }).click();
		await expect(page).toHaveURL(/\/login$/);
	});
}

test('first HTML exposes safe unique metadata and school links without JavaScript or forwarding cookies', async ({
	browser,
	request
}) => {
	const context = await browser.newContext({ javaScriptEnabled: false, ignoreHTTPSErrors: true });
	await context.addCookies([
		{ name: 'fixture-session', value: 'must-stay-on-frontend', url: baseUrl }
	]);
	const page = await context.newPage();
	const response = await page.goto(baseUrl + '/schools');
	expect(response?.status()).toBe(200);
	await expect(page.getByRole('link', { name: /โรงเรียนทดสอบ/ })).toBeVisible();
	expect(await page.locator('head title').count()).toBe(1);
	expect(await page.locator('meta[name="description"]').count()).toBe(1);
	expect(await page.locator('link[rel="canonical"]').getAttribute('href')).toBe(
		baseUrl + '/schools'
	);
	const structured = JSON.parse(
		(await page.locator('script[type="application/ld+json"]').textContent()) ?? '{}'
	);
	expect(structured.mainEntity.itemListElement[0].name).toBe(schoolName);
	expect(
		requests
			.filter((item) => item.path.startsWith('/api/v1/public/schools'))
			.every((item) => !item.cookie)
	).toBe(true);
	for (const path of ['/login', '/register', '/reset-password?token=fixture', '/missing']) {
		const result = await request.get(baseUrl + path);
		expect(result.headers()['x-robots-tag']).toBe('noindex');
	}
	const robots = await request.get(baseUrl + '/robots.txt');
	expect(robots.status()).toBe(200);
	expect(robots.headers()['content-type']).toContain('text/plain');
	const sitemap = await request.get(baseUrl + '/sitemap.xml');
	expect(sitemap.status()).toBe(200);
	expect(sitemap.headers()['content-type']).toContain('application/xml');
	expect(await sitemap.text()).toContain('<urlset');
	expect(await sitemap.text()).not.toContain('/login');
	await context.close();
});

test('unavailable, invalid or redirected public data returns an uncached retryable 503 while the homepage stays healthy', async ({
	request
}) => {
	for (const mode of ['unavailable', 'invalid', 'redirected']) {
		unavailable = mode === 'unavailable';
		invalid = mode === 'invalid';
		redirected = mode === 'redirected';
		const response = await request.get(baseUrl + '/schools');
		expect(response.status()).toBe(503);
		expect(response.headers()['retry-after']).toBe('60');
		expect(response.headers()['cache-control']).toContain('no-store');
		expect(response.headers()['x-robots-tag']).toBe('noindex');
		expect((await request.get(baseUrl)).status()).toBe(200);
	}
	expect(requests.some((item) => item.path === '/redirected-directory')).toBe(false);
	const before = requests.filter((item) => item.path.startsWith('/api/v1/public/schools')).length;
	expect((await request.get(baseUrl + '/schools?after=bad.host')).status()).toBe(400);
	expect(requests.filter((item) => item.path.startsWith('/api/v1/public/schools')).length).toBe(
		before
	);
});

test('an authenticated operator retains the dashboard entry on the public homepage', async ({
	page
}) => {
	authenticated = true;
	await page.goto(baseUrl);
	await expect(page.getByRole('link', { name: /บัญชีของฉัน/ }).first()).toHaveAttribute(
		'href',
		'/dashboard'
	);
	await expect(page).toHaveURL(baseUrl + '/');
});
