import { expect, test } from '@playwright/test';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';
import type { components } from '../../src/lib/api/generated/school-api.js';

type Schemas = components['schemas'];
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const entry = 'virtual:notification-device-lifecycle';
const prefix = '\0notification-device-lifecycle-stub:';
const stubs = new Map([
	[
		'#lib/stores/auth.js',
		`import {writable} from 'svelte/store';export const authStore=Object.assign(writable({isAuthenticated:true,isLoading:false,isUnavailable:false,user:{id:'fixture-user',user_type:'staff'}}),{sessionEpoch:1});`
	],
	['#lib/stores/work.js', 'export const workStore={};'],
	['#lib/realtime/auth-recovery.js', 'export const realtimeAuthRecovery=async()=>"stop";'],
	['svelte-sonner', 'export const toast={success(){}};'],
	[
		'#lib/api/client.js',
		`
		export const BACKEND_URL=location.origin;export const getSchoolSubdomainHint=()=>"fixture";
		export class ApiClientError extends Error {constructor(message,status){super(message);this.status=status;}}
		export const requireApiData=(response,message)=>{if(!response.success||response.data===undefined)throw new ApiClientError(response.error||message,response.status);return response.data;};
		const request=async(method,url,body)=>{const response=await fetch(url,{method,headers:{'Content-Type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)});return {...await response.json(),status:response.status};};
		export const apiClient={get:url=>request('GET',url),post:(url,body)=>request('POST',url,body),delete:url=>request('DELETE',url)};
	`
	],
	[
		'#lib/pwa/service-worker-registration.js',
		`
		let subscription;window.__unsubscribed=0;
		const create=()=>({endpoint:'https://push.invalid/fixture',options:{applicationServerKey:new Uint8Array([1,2,3]).buffer},getKey:()=>new Uint8Array([1,2]).buffer,async unsubscribe(){window.__unsubscribed++;if(subscription===this)subscription=null;return true;}});
		subscription=create();
		export async function getServiceWorkerRegistration(){return {pushManager:{async getSubscription(){return subscription;},async subscribe(){subscription=create();return subscription;}},async getNotifications(){return [];}};}
	`
	]
]);
let device: Schemas['PushDevice'] | null = null;
const registrations: Schemas['RegisterPushDeviceRequest'][] = [];
let nextFailure: { status: number; error: string } | null = null;
let pushAvailable = true;
function plugin(): Plugin {
	return {
		name: 'notification-device-lifecycle-fixture',
		enforce: 'pre',
		resolveId(id) {
			if (id === entry) return `\0${entry}`;
			if (stubs.has(id)) return prefix + [...stubs.keys()].indexOf(id);
		},
		load(id) {
			if (id.startsWith(prefix))
				return stubs.get([...stubs.keys()][Number(id.slice(prefix.length))]);
			if (id !== `\0${entry}`) return;
			return `import {notificationStore} from '/src/lib/stores/notification.ts';import {readBrowserState} from '/src/lib/notifications/browser-state.ts';import {listPushDevices,unregisterPushDevice} from '/src/lib/api/notifications.ts';window.__deviceRuntime={enable:()=>notificationStore.enablePushFromUserAction(),authSync:()=>notificationStore.syncExistingPushSubscription(),state:readBrowserState,async remoteRevoke(){const [device]=await listPushDevices();await unregisterPushDevice(device.id,device.version);}};`;
		},
		configureServer(server) {
			server.middlewares.use(async (request, response, next) => {
				const url = new URL(request.url ?? '/', 'http://fixture');
				const send = (data: object, status = 200) => {
					response.statusCode = status;
					response.setHeader('Content-Type', 'application/json');
					response.end(
						JSON.stringify(status === 200 ? { success: true, data } : { success: false, ...data })
					);
				};
				if (url.pathname === '/__notification-device-lifecycle') {
					response.setHeader('Content-Type', 'text/html');
					response.end(
						`<main>Local notification lifecycle fixture</main><script type="module" src="/@id/${entry}"></script>`
					);
					return;
				}
				if (url.pathname === '/__notification-device-state') {
					send({ registrations, device });
					return;
				}
				if (url.pathname === '/api/notifications/push/config') {
					send({
						available: pushAvailable,
						public_key: pushAvailable ? 'AQID' : null,
						key_id: 'key-1',
						max_devices: 20,
						device_expiry_days: 90
					});
					return;
				}
				if (url.pathname === '/api/notifications/push/devices') {
					if (request.method === 'GET') {
						send({ items: device ? [device] : [] });
						return;
					}
					let body = '';
					for await (const chunk of request) body += chunk;
					const input: Schemas['RegisterPushDeviceRequest'] = JSON.parse(body);
					registrations.push(input);
					if (nextFailure) {
						const failure = nextFailure;
						nextFailure = null;
						send({ error: failure.error }, failure.status);
						return;
					}
					if (
						input.expected_version &&
						(!device ||
							device.installation_id !== input.installation_id ||
							device.version !== input.expected_version)
					) {
						send({ error: 'push_binding_changed' }, 409);
						return;
					}
					device = {
						id: device?.id ?? randomUUID(),
						installation_id: input.installation_id,
						label: input.label,
						version: device?.version ?? randomUUID(),
						binding_token: device?.binding_token ?? randomUUID(),
						key_id: input.key_id,
						last_confirmed_at: new Date().toISOString(),
						expires_at: new Date(Date.now() + 90 * 86400_000).toISOString()
					};
					send(device);
					return;
				}
				if (
					url.pathname.startsWith('/api/notifications/push/devices/') &&
					request.method === 'DELETE'
				) {
					if (
						device &&
						device.id === url.pathname.split('/').at(-1) &&
						device.version === url.searchParams.get('version')
					)
						device = null;
					send({});
					return;
				}
				next();
			});
		}
	};
}
let server: ViteDevServer;
let origin: string;
test.beforeAll(async () => {
	server = await createServer({
		root,
		cacheDir: path.join(root, 'node_modules/.vite-notification-device-lifecycle-test'),
		logLevel: 'error',
		plugins: [plugin()],
		server: { host: '127.0.0.1', port: 0 }
	});
	await server.listen();
	const address = server.httpServer?.address();
	if (!address || typeof address === 'string') throw new Error('fixture did not start');
	origin = `http://127.0.0.1:${address.port}`;
});
test.afterAll(async () => {
	await server.close();
});
test('remote DELETE survives the next auth sync until explicit device enable', async ({
	page,
	context
}) => {
	await context.grantPermissions(['notifications'], { origin });
	// PushManager is synthetic here; keep its companion permission deterministic without OS prompts.
	await page.addInitScript(() =>
		Object.defineProperty(Notification, 'permission', { get: () => 'granted' })
	);
	await page.goto(`${origin}/__notification-device-lifecycle`);
	await page.waitForFunction('Boolean(window.__deviceRuntime)');
	expect(await page.evaluate('window.__deviceRuntime.authSync()')).toBe(false);
	expect(registrations).toHaveLength(0);
	expect(await page.evaluate('window.__deviceRuntime.enable()')).toBe(true);
	const version = device!.version;
	expect(registrations[0].expected_version).toBeUndefined();
	const refreshed = await page.evaluate('window.__deviceRuntime.authSync()');
	expect(
		refreshed,
		JSON.stringify({
			state: await page.evaluate('window.__deviceRuntime.state()'),
			permission: await page.evaluate('Notification.permission'),
			registrations
		})
	).toBe(true);
	expect(registrations[1].expected_version).toBe(version);
	for (const failure of [
		{ status: 409, error: 'push_key_changed' },
		{ status: 503, error: 'unavailable' }
	]) {
		nextFailure = failure;
		expect(await page.evaluate('window.__deviceRuntime.authSync()')).toBe(false);
		expect(
			await page.evaluate('window.__deviceRuntime.state().then(state=>state.binding?.version)')
		).toBe(version);
		expect(await page.evaluate('window.__unsubscribed')).toBe(0);
	}
	pushAvailable = false;
	expect(await page.evaluate('window.__deviceRuntime.authSync()')).toBe(false);
	expect(
		await page.evaluate('window.__deviceRuntime.state().then(state=>state.binding?.version)')
	).toBe(version);
	pushAvailable = true;
	const attemptsBeforeRevoke = registrations.length;
	await page.evaluate('window.__deviceRuntime.remoteRevoke()');
	expect(device).toBeNull();
	expect(await page.evaluate('window.__deviceRuntime.authSync()')).toBe(false);
	expect(device).toBeNull();
	expect(registrations[attemptsBeforeRevoke].expected_version).toBe(version);
	expect(
		await page.evaluate('window.__deviceRuntime.state().then(state=>state.binding)')
	).toBeNull();
	expect(await page.evaluate('window.__unsubscribed')).toBe(1);
	expect(await page.evaluate('window.__deviceRuntime.authSync()')).toBe(false);
	expect(registrations).toHaveLength(attemptsBeforeRevoke + 1);
	expect(await page.evaluate('window.__deviceRuntime.enable()')).toBe(true);
	expect(registrations[attemptsBeforeRevoke + 1].expected_version).toBeUndefined();
	expect(device).not.toBeNull();
	expect(device!.version).not.toBe(version);
});
