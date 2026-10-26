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
	const icon = await readFile('static/schoolorbit-192.png');
	const owner = ts.transpileModule(
		await readFile('src/lib/pwa/service-worker-registration.ts', 'utf8'),
		{ compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } }
	).outputText;
	const browserState = ts.transpileModule(
		await readFile('src/lib/notifications/browser-state.ts', 'utf8'),
		{ compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } }
	).outputText;
	server = createServer((request, response) => {
		response.setHeader('Cache-Control', 'no-store');
		const url = new URL(request.url ?? '/', 'http://localhost');
		if (url.pathname === '/service-worker.js') {
			response.setHeader('Content-Type', 'text/javascript');
			response.end(legacy ? legacyWorker : worker);
		} else if (url.pathname === '/app-icon/192.png') {
			response.setHeader('Content-Type', 'image/png');
			response.end(icon);
		} else if (url.pathname === '/_app/env.js') {
			response.setHeader('Content-Type', 'text/javascript');
			response.end('export const env = {};');
		} else if (url.pathname === '/state.js') {
			response.setHeader('Content-Type', 'text/javascript');
			response.end(browserState);
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

test('Chromium: actual built worker enforces binding, freshness, deduplication and generic navigation', async () => {
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
		const payload = {
			id: 'fresh',
			title: 'private title',
			body: 'private body',
			link: '/staff/academic/delivery',
			device_id: 'device',
			device_version: 'version',
			binding_token: 'token',
			expires_at: new Date(Date.now() + 60000).toISOString()
		};
		const push = (data: typeof payload) =>
			worker.evaluate(async (data) => {
				let work: Promise<unknown> | undefined;
				const PushConstructor = (
					globalThis as unknown as {
						PushEvent: new (type: string, init: { data: string }) => Event;
					}
				).PushEvent;
				const event = new PushConstructor('push', { data: JSON.stringify(data) });
				Object.defineProperty(event, 'waitUntil', {
					value: (promise: Promise<unknown>) => {
						work = promise;
					}
				});
				dispatchEvent(event);
				await work;
			}, data);
		const notifications = () =>
			page.evaluate(async () =>
				(await (await navigator.serviceWorker.ready).getNotifications()).map((item) => ({
					title: item.title,
					body: item.body,
					icon: item.icon,
					link: item.data.link,
					tag: item.tag,
					renotify: Reflect.get(item, 'renotify')
				}))
			);
		await push(payload);
		expect(await notifications()).toEqual([]);
		await page.evaluate(async () => {
			const state: typeof import('../../src/lib/notifications/browser-state.js') = await import(
				`${location.origin}/state.js`
			);
			const current = await state.beginBinding('fixture-user', 'intent');
			await state.saveBinding({
				identity: 'fixture-user',
				intent: 'intent',
				id: 'device',
				version: 'version',
				binding_token: 'token',
				installation_id: current.installation_id,
				expires_at: new Date(Date.now() + 86400_000).toISOString()
			});
		});
		await push({ ...payload, id: 'expired', expires_at: new Date(Date.now() - 1).toISOString() });
		await push({ ...payload, id: 'old-account', binding_token: 'old-token' });
		await push({ ...payload, id: 'old-device', device_version: 'old-version' });
		expect(await notifications()).toEqual([]);
		await push(payload);
		await push(payload);
		expect(await notifications()).toEqual([
			{
				title: 'SchoolOrbit',
				body: 'มีการแจ้งเตือนใหม่ กรุณาเปิดดูในกล่องแจ้งเตือน',
				link: `${origin}/staff/academic/delivery`,
				icon: `${origin}/app-icon/192.png`,
				tag: 'schoolorbit:fresh',
				renotify: false
			}
		]);
		await page.evaluate(async () => {
			const state: typeof import('../../src/lib/notifications/browser-state.js') = await import(
				`${location.origin}/state.js`
			);
			await state.clearBinding('fixture-user');
			for (const notification of await (await navigator.serviceWorker.ready).getNotifications())
				notification.close();
		});
		await push({ ...payload, id: 'queued-after-logout' });
		expect(await notifications()).toEqual([]);
	} finally {
		await browser.close();
	}
});
