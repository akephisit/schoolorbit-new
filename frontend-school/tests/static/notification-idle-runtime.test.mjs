import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';
import { writable, get } from 'svelte/store';
import { createVisibilityIdle } from '../../src/lib/realtime/visibility-idle.ts';
import { realtimeAuthRecovery } from '../../src/lib/realtime/auth-recovery.ts';

test('notification hidden pause suppresses recovery and resumes authoritative state once', async () => {
	let hidden = false;
	const listeners = new Set();
	const timers = new Map();
	const sources = [];
	let nextTimer = 0;
	let authCalls = 0;
	const authOptions = [];
	let workCalls = 0;
	let notificationCalls = 0;
	let completeSnapshot;
	const setTimer = (callback, delay) => {
		timers.set(++nextTimer, { callback, delay });
		return nextTimer;
	};
	const clearTimer = (id) => timers.delete(id);
	class Source {
		static CLOSED = 2;
		readyState = 1;
		events = {};
		constructor() {
			sources.push(this);
		}
		close() {
			this.readyState = 2;
		}
		addEventListener(name, listener) {
			this.events[name] = listener;
		}
	}
	const dependencies = {
		'$env/static/public': { PUBLIC_VAPID_KEY: '' },
		'$lib/api/client': {
			BACKEND_URL: 'https://school.example',
			getSchoolSubdomainHint: () => null,
			apiClient: {
				get: async () => {
					notificationCalls++;
					if (notificationCalls === 1)
						return new Promise((resolve) => {
							completeSnapshot = resolve;
						});
					return { success: true, data: { items: [], unread_count: 4 } };
				}
			}
		},
		'$lib/realtime/auth-recovery': { realtimeAuthRecovery },
		'$lib/realtime/visibility-idle': {
			createVisibilityIdle,
			browserVisibilityDependencies: () => ({
				isHidden: () => hidden,
				setTimer,
				clearTimer,
				addListener: (l) => listeners.add(l),
				removeListener: (l) => listeners.delete(l)
			})
		},
		'$lib/stores/work': {
			workStore: {
				refreshSilently: async () => {
					workCalls++;
					return true;
				}
			}
		},
		'$lib/api/auth': {
			authAPI: {
				refreshCurrentUser: async (options) => {
					authOptions.push(options);
					authCalls++;
					return 'authenticated';
				}
			}
		},
		'svelte-sonner': { toast: { success() {} } },
		'svelte/store': { writable }
	};
	const source = await readFile(
		new URL('../../src/lib/stores/notification.ts', import.meta.url),
		'utf8'
	);
	const compiled = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
	}).outputText;
	const exports = {};
	new Function('require', 'exports', 'EventSource', 'setTimeout', 'clearTimeout', compiled)(
		(id) => {
			assert.ok(id in dependencies, id);
			return dependencies[id];
		},
		exports,
		Source,
		setTimer,
		clearTimer
	);
	const store = exports.notificationStore;
	store.initSSE();
	assert.equal(sources.length, 1);
	const staleError = sources[0].onerror;
	hidden = true;
	for (const listener of listeners) listener();
	const idleTimer = [...timers.entries()].find(([, timer]) => timer.delay === 60_000);
	assert.ok(idleTimer, 'hidden tab must schedule idle pause');
	timers.delete(idleTimer[0]);
	idleTimer[1].callback();
	assert.equal(sources[0].readyState, Source.CLOSED);
	staleError();
	store.initSSE();
	await new Promise((resolve) => setImmediate(resolve));
	assert.equal(authCalls, 0);
	assert.equal(sources.length, 1);
	hidden = false;
	for (const listener of listeners) listener();
	await new Promise((resolve) => setImmediate(resolve));
	assert.equal(authCalls, 1);
	assert.equal(workCalls, 0, 'snapshot must wait until subscribed');
	assert.equal(notificationCalls, 0);
	for (const [id, timer] of [...timers]) {
		timers.delete(id);
		timer.callback();
	}
	assert.equal(sources.length, 2);
	sources[1].onopen();
	await new Promise((resolve) => setImmediate(resolve));
	assert.equal(authCalls, 2);
	assert.equal(
		authOptions[1].invalidate,
		true,
		'post-subscribe auth cannot reuse pre-subscribe snapshot'
	);
	assert.equal(workCalls, 1);
	assert.equal(notificationCalls, 1);
	// An event received during HTTP catch-up must not be erased by the older snapshot.
	sources[1].onmessage({ data: JSON.stringify({ id: 'new', title: 'New', message: 'New' }) });
	completeSnapshot({ success: true, data: { items: [], unread_count: 3 } });
	await new Promise((resolve) => setImmediate(resolve));
	assert.equal(get(store).unreadCount, 1);
	assert.equal(get(store).notifications[0].id, 'new');
	assert.equal(timers.size, 1, 'a changed snapshot schedules bounded catch-up retry');
	for (const [id, timer] of [...timers]) {
		timers.delete(id);
		timer.callback();
	}
	await new Promise((resolve) => setImmediate(resolve));
	assert.equal(notificationCalls, 2);
	assert.equal(get(store).unreadCount, 4);
	store.closeSSE();
	assert.equal(listeners.size, 0);
	assert.equal(timers.size, 0);
});
