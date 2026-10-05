import { test, expect, chromium, webkit } from '@playwright/test';
import { createServer, type Server } from 'node:http';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';

// Serve the built worker unchanged. Only the small registration owner is transpiled for this harness.
let server: Server;
let origin: string;
let legacy = false;
let networkReads = 0;
const legacyWorker = `self.addEventListener('install',e=>e.waitUntil(self.skipWaiting()));self.addEventListener('activate',e=>e.waitUntil(self.clients.claim()));self.addEventListener('fetch',e=>e.respondWith(Promise.reject(new Error('legacy interception failed'))));`;

test.beforeAll(async () => {
	const worker = await readFile('.svelte-kit/output/client/service-worker.js', 'utf8');
	const owner = ts.transpileModule(
		await readFile('src/lib/pwa/service-worker-registration.ts', 'utf8'),
		{ compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } }
	).outputText;
	server = createServer((request, response) => {
		response.setHeader('Cache-Control', 'no-store');
		const url = new URL(request.url ?? '/', 'http://localhost');
		if (url.pathname === '/service-worker.js') {
			response.setHeader('Content-Type', 'text/javascript');
			response.end(legacy ? legacyWorker : worker);
		} else if (url.pathname === '/_app/env.js') {
			response.setHeader('Content-Type', 'text/javascript');
			response.end('export const env = {};');
		} else if (url.pathname === '/owner.js') {
			response.setHeader('Content-Type', 'text/javascript');
			response.end(owner);
		} else {
			networkReads++;
			response.setHeader('Content-Type', 'text/html; charset=utf-8');
			response.end(
				`<input aria-label="ข้อมูลที่กรอก"><p>หน้าเว็บโหลดผ่านเครือข่าย</p><script type="module">import {getServiceWorkerRegistration} from '/owner.js';window.startWorker=getServiceWorkerRegistration;window.ready=getServiceWorkerRegistration();window.ready.catch(()=>{});</script>`
			);
		}
	});
	await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
	const address = server.address();
	if (!address || typeof address === 'string') throw new Error('test server missing TCP address');
	origin = `http://127.0.0.1:${address.port}`;
});

test.afterAll(async () => {
	await new Promise<void>((resolve, reject) =>
		server.close((error) => (error ? reject(error) : resolve()))
	);
});

for (const [name, browserType] of [
	['Chromium', chromium],
	['WebKit', webkit]
] as const) {
	test(`${name}: built module worker supports direct navigation, reload and recovery after network failure`, async () => {
		legacy = false;
		const browser = await browserType.launch();
		const context = await browser.newContext({ serviceWorkers: 'allow' });
		try {
			const page = await context.newPage();
			await page.goto(`${origin}/staff/academic/delivery?direct=1`);
			await page.evaluate('window.ready');
			expect(await page.evaluate('window.startWorker() === window.startWorker()')).toBe(true);
			const before = networkReads;
			await page.reload();
			await page.evaluate('window.ready');
			await expect(page.getByText('หน้าเว็บโหลดผ่านเครือข่าย')).toBeVisible();
			expect(networkReads).toBeGreaterThan(before);
			const registration = await page.evaluate(async () => {
				const registrations = await navigator.serviceWorker.getRegistrations();
				return registrations.map((item) => ({ scope: item.scope, script: item.active?.scriptURL }));
			});
			expect(registration).toEqual([
				{ scope: `${origin}/`, script: `${origin}/service-worker.js` }
			]);
			await context.setOffline(true);
			await expect(page.goto(`${origin}/staff/academic/delivery?offline=1`)).rejects.toThrow();
			await context.setOffline(false);
			await page.goto(`${origin}/staff/academic/delivery?recovered=1`);
			await expect(page.getByText('หน้าเว็บโหลดผ่านเครือข่าย')).toBeVisible();
		} finally {
			await browser.close();
		}
	});

	test(`${name}: updates the old intercepted worker in place and preserves entered data`, async () => {
		legacy = true;
		const browser = await browserType.launch();
		const context = await browser.newContext({ serviceWorkers: 'allow' });
		try {
			const page = await context.newPage();
			await page.goto(`${origin}/install`);
			await page.evaluate('window.ready');
			await page.getByLabel('ข้อมูลที่กรอก').fill('ข้อความยังไม่บันทึก');
			await page.evaluate(() => localStorage.setItem('user-draft', 'retained'));
			legacy = false;
			await page.evaluate(async () => {
				const registration = await navigator.serviceWorker.ready;
				const changed = new Promise<void>((resolve) =>
					navigator.serviceWorker.addEventListener('controllerchange', () => resolve(), {
						once: true
					})
				);
				await registration.update();
				await changed;
			});
			await expect(page.getByLabel('ข้อมูลที่กรอก')).toHaveValue('ข้อความยังไม่บันทึก');
			expect(await page.evaluate(() => localStorage.getItem('user-draft'))).toBe('retained');
			expect(
				await page.evaluate(async () => (await navigator.serviceWorker.getRegistrations()).length)
			).toBe(1);
			await page.goto(`${origin}/staff/academic/delivery?updated=1`);
			await expect(page.getByText('หน้าเว็บโหลดผ่านเครือข่าย')).toBeVisible();
		} finally {
			await browser.close();
			legacy = false;
		}
	});
}

test('Chromium: actual built worker displays push notification with its navigation target', async () => {
	legacy = false;
	const browser = await chromium.launch({ channel: 'chromium' });
	const context = await browser.newContext({
		serviceWorkers: 'allow',
		permissions: ['notifications']
	});
	try {
		await context.grantPermissions(['notifications'], { origin });
		const page = await context.newPage();
		await page.goto(`${origin}/push`);
		await page.evaluate('window.ready');
		const worker = context.serviceWorkers()[0];
		expect(worker).toBeTruthy();
		await worker.evaluate(async () => {
			let work: Promise<unknown> | undefined;
			const PushConstructor = (
				globalThis as unknown as { PushEvent: new (type: string, init: { data: string }) => Event }
			).PushEvent;
			const event = new PushConstructor('push', {
				data: JSON.stringify({
					title: 'แจ้งเตือนทดสอบ',
					body: 'ข้อความจากโรงเรียน',
					link: '/staff/academic/delivery'
				})
			});
			Object.defineProperty(event, 'waitUntil', {
				value: (promise: Promise<unknown>) => {
					work = promise;
				}
			});
			dispatchEvent(event);
			await work;
		});
		const notifications = await page.evaluate(async () =>
			(await (await navigator.serviceWorker.ready).getNotifications()).map((item) => ({
				title: item.title,
				body: item.body,
				link: item.data.link
			}))
		);
		expect(notifications).toEqual([
			{ title: 'แจ้งเตือนทดสอบ', body: 'ข้อความจากโรงเรียน', link: '/staff/academic/delivery' }
		]);
	} finally {
		await browser.close();
	}
});
