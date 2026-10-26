import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';
import { writable, get } from 'svelte/store';

const source = await readFile(
	new URL('../../src/lib/stores/notification.ts', import.meta.url),
	'utf8'
);
const compiled = ts.transpileModule(source, {
	compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
}).outputText;
export const flush = () => new Promise((resolve) => setImmediate(resolve));
export function deferred() {
	let resolve;
	const promise = new Promise((done) => {
		resolve = done;
	});
	return { promise, resolve };
}
export function deviceHarness(options = {}) {
	const authStore = Object.assign(
		writable({ isAuthenticated: true, user: { id: 'user-a', user_type: 'staff' } }),
		{ sessionEpoch: 1 }
	);
	const state = { installation_id: 'installation', identity: null, intent: '', binding: null };
	const calls = {
		register: [],
		revoke: [],
		subscribe: [],
		unsubscribe: 0,
		permission: 0,
		close: 0,
		toasts: []
	};
	const key = 'AQID';
	const config = {
		available: true,
		public_key: key,
		key_id: 'key-1',
		max_devices: 20,
		device_expiry_days: 90,
		...options.config
	};
	let subscription =
		options.subscription === false ? null : makeSubscription(options.oldKey ? [8, 9] : [1, 2, 3]);
	function makeSubscription(bytes) {
		return {
			endpoint: 'https://push.invalid/sub',
			options: { applicationServerKey: new Uint8Array(bytes).buffer },
			getKey: () => new Uint8Array([1, 2]).buffer,
			async unsubscribe() {
				calls.unsubscribe++;
				subscription = null;
				return true;
			}
		};
	}
	const registration = {
		pushManager: {
			async getSubscription() {
				return subscription;
			},
			async subscribe(input) {
				calls.subscribe.push(input);
				subscription = makeSubscription([...input.applicationServerKey]);
				return subscription;
			}
		},
		async getNotifications() {
			return [
				{
					close() {
						calls.close++;
					}
				}
			];
		}
	};
	const devices = [];
	const browserState = {
		watchIdentityChanges: () => () => {},
		async beginBinding(identity, intent) {
			if (state.identity !== identity) state.binding = null;
			Object.assign(state, { identity, intent });
			return { ...state };
		},
		async readBrowserState() {
			return { ...state };
		},
		async saveBinding(binding) {
			if (binding.identity !== state.identity || binding.intent !== state.intent) return false;
			state.binding = binding;
			return true;
		},
		async clearBinding(identity, intent) {
			if (identity !== state.identity || (intent && intent !== state.intent)) return null;
			const binding = state.binding;
			Object.assign(state, { binding: null, identity: null, intent: 'cleared' });
			return binding;
		},
		async claimPresentation() {
			return true;
		}
	};
	const dependencies = {
		'#lib/notifications/browser-state.js': browserState,
		'#lib/api/notifications.js': {
			async getPushConfig() {
				return config;
			},
			async listPushDevices() {
				return devices;
			},
			async registerPushDevice(body) {
				calls.register.push(body);
				if (options.register) return options.register(body, calls.register.length);
				const device = {
					...body,
					id: 'device-1',
					version: 'version-1',
					binding_token: 'token-1',
					last_confirmed_at: new Date().toISOString(),
					expires_at: new Date(Date.now() + 86400_000).toISOString()
				};
				devices.splice(0, devices.length, device);
				return device;
			},
			async unregisterPushDevice(id, version) {
				calls.revoke.push({ id, version });
				devices.splice(0);
			}
		},
		'#lib/api/client.js': {
			BACKEND_URL: 'https://api.invalid',
			getSchoolSubdomainHint: () => 'school',
			apiClient: {}
		},
		'#lib/stores/auth.js': { authStore },
		'#lib/stores/work.js': {},
		'#lib/realtime/auth-recovery.js': {},
		'#lib/realtime/visibility-idle.js': {
			createVisibilityIdle: () => ({ stop() {}, start() {}, paused: false }),
			browserVisibilityDependencies: () => ({})
		},
		'#lib/pwa/service-worker-registration.js': {
			async getServiceWorkerRegistration() {
				return registration;
			}
		},
		'svelte/store': { writable, get },
		'svelte-sonner': { toast: { success: (...args) => calls.toasts.push(args) } }
	};
	const exports = {};
	new Function(
		'require',
		'exports',
		'navigator',
		'window',
		'Notification',
		'document',
		'console',
		compiled
	)(
		(id) => {
			assert.ok(id in dependencies, id);
			return dependencies[id];
		},
		exports,
		{
			userAgent: 'test',
			maxTouchPoints: 0,
			serviceWorker: { addEventListener() {}, removeEventListener() {} }
		},
		{ PushManager: {}, Notification: {}, atob, btoa, matchMedia: () => ({ matches: false }) },
		{
			permission: 'granted',
			async requestPermission() {
				calls.permission++;
				return 'granted';
			}
		},
		{ hidden: false, hasFocus: () => true },
		{
			log() {},
			warn() {},
			error: (...args) => {
				if (!options.allowErrors) assert.fail(args.join(' '));
			}
		}
	);
	const store = exports.notificationStore;
	return {
		store,
		state,
		calls,
		config,
		authStore,
		devices,
		switchUser(id) {
			authStore.sessionEpoch++;
			authStore.set({ isAuthenticated: true, user: { id, user_type: 'staff' } });
			store.bindIdentity();
		}
	};
}
