import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const virtual = 'virtual:notification-settings-test';
const prefix = '\0notification-settings-stub:';
const stubs = new Map([
	[
		'#lib/stores/auth.js',
		`import {writable} from 'svelte/store'; export const authStore=Object.assign(writable({isAuthenticated:true,user:{id:'fixture'}}),{sessionEpoch:1});`
	],
	[
		'svelte-sonner',
		`export const toast={success:(text)=>window.__toasts.push(text),error:(text)=>window.__toasts.push(text)};`
	],
	[
		'#lib/stores/notification.js',
		`
		const device=(id,label)=>({id,installation_id:id,label,version:'v1',binding_token:'token',key_id:'key',last_confirmed_at:'2026-10-20T10:00:00Z',expires_at:'2027-01-20T10:00:00Z'});
		let status={supported:true,permission:'granted',hasSubscription:true,isIOS:false,isStandalone:false,serverAvailable:true,synced:true,currentDeviceId:'current',devices:[device('current','เว็บเบราว์เซอร์'),device('other','โทรศัพท์ของฉัน')],config:{available:true,public_key:'AQID',key_id:'key',max_devices:20,device_expiry_days:90},error:null};
		export const notificationStore={
		 async getPushStatus(){if(window.__scenario==='unavailable')return {...status,serverAvailable:false,synced:false}; if(window.__scenario==='error')return {...status,synced:false,devices:[],config:null,error:'ไม่สามารถตรวจสอบการเชื่อมต่อกับระบบได้'};return structuredClone(status);},
		 async enablePushFromUserAction(){window.__actions.push('enable');status.synced=true;return true;},
		 async revokePushDevice(device){window.__actions.push('revoke:'+device.id);status.devices=status.devices.filter(d=>d.id!==device.id);if(device.id==='current')status.synced=false;}
		};
	`
	]
]);
function harnessPlugin(): Plugin {
	return {
		name: 'notification-settings-test',
		enforce: 'pre',
		resolveId(id) {
			if (id === virtual) return `\0${virtual}`;
			if (stubs.has(id)) return `${prefix}${id}`;
		},
		load(id) {
			if (id.startsWith(prefix)) return stubs.get(id.slice(prefix.length));
			if (id !== `\0${virtual}`) return;
			return `import {mount} from 'svelte'; import '/src/routes/layout.css'; import Settings from '/src/lib/components/settings/PushNotificationSettings.svelte'; import * as state from '/src/lib/notifications/browser-state.ts'; window.__state=state; window.__actions=[];window.__toasts=[];mount(Settings,{target:document.getElementById('app')});`;
		},
		configureServer(server) {
			server.middlewares.use((request, response, next) => {
				if (new URL(request.url ?? '/', 'http://fixture').pathname !== '/__notification-settings')
					return next();
				response.setHeader('Content-Type', 'text/html');
				response.end(
					`<html><meta name="viewport" content="width=device-width,initial-scale=1"><body><main id="app" style="max-width:60rem;margin:1rem auto"></main><script type="module" src="/@id/${virtual}"></script></body></html>`
				);
			});
		}
	};
}
let server: ViteDevServer;
let origin: string;
test.beforeAll(async () => {
	server = await createServer({
		root: frontendRoot,
		cacheDir: path.join(frontendRoot, 'node_modules/.vite-notification-settings-test'),
		logLevel: 'error',
		plugins: [harnessPlugin()],
		server: { host: '127.0.0.1', port: 0 }
	});
	await server.listen();
	const address = server.httpServer?.address();
	if (!address || typeof address === 'string') throw new Error('missing fixture address');
	origin = `http://127.0.0.1:${address.port}`;
});
test.afterAll(async () => {
	await server.close();
});

for (const width of [390, 1280]) {
	test(`settings show device lifecycle at ${width}px`, async ({ page }) => {
		await page.setViewportSize({ width, height: 900 });
		await page.goto(`${origin}/__notification-settings`);
		await expect(page.getByText('เชื่อมต่อแล้ว', { exact: true }).first()).toBeVisible();
		await expect(page.getByText('อุปกรณ์ของคุณ (2/20)')).toBeVisible();
		await expect(page.getByText('เว็บเบราว์เซอร์ (เครื่องนี้)')).toBeVisible();
		await expect(page.getByText(/ยืนยันล่าสุด/).first()).toBeVisible();
		await page.getByRole('button', { name: 'ยกเลิกอุปกรณ์', exact: true }).nth(1).click();
		await expect(page.getByText('อุปกรณ์ของคุณ (1/20)')).toBeVisible();
		await expect(page.getByText('โทรศัพท์ของฉัน')).toHaveCount(0);
		await expect(page.getByText('เว็บเบราว์เซอร์ (เครื่องนี้)')).toBeVisible();
		expect(await page.evaluate('document.documentElement.scrollWidth<=window.innerWidth')).toBe(
			true
		);
		await page.getByRole('button', { name: 'ยกเลิกอุปกรณ์', exact: true }).click();
		await expect(page.getByText('ต้องซิงก์', { exact: true })).toBeVisible();
		await page.getByRole('button', { name: 'เปิด/ซิงก์การแจ้งเตือน' }).click();
		expect(await page.evaluate('window.__actions')).toEqual([
			'revoke:other',
			'revoke:current',
			'enable'
		]);
	});
}
test('unavailable and error configurations never claim the browser subscription is connected', async ({
	page
}) => {
	await page.addInitScript(() => {
		Object.assign(window, { __scenario: 'unavailable' });
	});
	await page.goto(`${origin}/__notification-settings`);
	await expect(page.getByText('ระบบยังไม่พร้อม', { exact: true })).toBeVisible();
	await expect(page.getByText(/กล่องแจ้งเตือนยังใช้งานได้/)).toBeVisible();
	await expect(page.getByRole('button', { name: 'เปิด/ซิงก์การแจ้งเตือน' })).toBeDisabled();
	await page.addInitScript(() => {
		Object.assign(window, { __scenario: 'error' });
	});
	await page.reload();
	await expect(page.getByText('ตรวจสอบไม่ได้', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'ลองตรวจสอบอีกครั้ง' })).toBeVisible();
});
test('real IndexedDB serializes cross-tab presentation and rejects stale binding writes after logout', async ({
	page,
	context
}) => {
	await page.goto(`${origin}/__notification-settings`);
	await page.waitForFunction('Boolean(window.__state)');
	const other = await context.newPage();
	await other.goto(`${origin}/__notification-settings`);
	await other.waitForFunction('Boolean(window.__state)');
	const installation = await page.evaluate('window.__state.beginBinding("school:user","intent-a")');
	const otherInstallation = await other.evaluate('window.__state.readBrowserState()');
	expect(otherInstallation.installation_id).toBe(installation.installation_id);
	const claims = await Promise.all([
		page.evaluate('window.__state.claimPresentation("shared-id",Date.now()+60000)'),
		other.evaluate('window.__state.claimPresentation("shared-id",Date.now()+60000)')
	]);
	expect(claims.filter(Boolean)).toHaveLength(1);
	await other.evaluate('window.__state.clearBinding("school:user")');
	const staleSaved = await page.evaluate(
		'window.__state.saveBinding({identity:"school:user",intent:"intent-a",id:"old",version:"v1",binding_token:"token",installation_id:"unused",expires_at:new Date(Date.now()+60000).toISOString()})'
	);
	expect(staleSaved).toBe(false);
	await page.evaluate('window.__state.beginBinding("school:new-user","intent-b")');
	const staleAccount = await other.evaluate(
		'window.__state.saveBinding({identity:"school:user",intent:"intent-a",id:"old",version:"v1",binding_token:"token",installation_id:"unused",expires_at:new Date(Date.now()+60000).toISOString()})'
	);
	expect(staleAccount).toBe(false);
	expect((await other.evaluate('window.__state.readBrowserState()')).identity).toBe(
		'school:new-user'
	);
	await other.close();
});
