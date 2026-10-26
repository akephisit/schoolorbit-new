import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';
import { writable, get } from 'svelte/store';
import { createVisibilityIdle } from '../../src/lib/realtime/visibility-idle.ts';
import { realtimeAuthRecovery } from '../../src/lib/realtime/auth-recovery.ts';

const flush = () => new Promise((resolve) => setImmediate(resolve));
const notificationSource = await readFile(
	new URL('../../src/lib/stores/notification.ts', import.meta.url),
	'utf8'
);
const workSource = await readFile(new URL('../../src/lib/stores/work.ts', import.meta.url), 'utf8');

function deferred() {
	let resolve;
	const promise = new Promise((complete) => {
		resolve = complete;
	});
	return { promise, resolve };
}

function loadSource(source, dependencies, globals = {}) {
	const compiled = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
	}).outputText;
	const exports = {};
	new Function('require', 'exports', ...Object.keys(globals), compiled)(
		(id) => {
			assert.ok(id in dependencies, id);
			return dependencies[id];
		},
		exports,
		...Object.values(globals)
	);
	return exports;
}

function harness(options = {}) {
	const sources = [];
	const toasts = [];
	const claims = options.claims ?? new Set();
	const authStore = Object.assign(
		writable({ isAuthenticated: true, user: { id: 'first', user_type: 'staff' } }),
		{ sessionEpoch: 1 }
	);
	const timers = new Map();
	const authOptions = [];
	let timerId = 0;
	let notificationCalls = 0;
	let workCalls = 0;
	const setTimer = (callback, delay) => {
		timers.set(++timerId, { callback, delay });
		return timerId;
	};
	const clearTimer = (id) => timers.delete(id);
	class Source {
		static CLOSED = 2;
		readyState = 0;
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
		open() {
			this.readyState = 1;
			this.onopen();
		}
		error(readyState = 0) {
			this.readyState = readyState;
			this.onerror();
		}
		lag() {
			assert.equal(typeof this.events.reconcile_required, 'function');
			this.events.reconcile_required({ data: '' });
		}
	}
	const { workStore } = loadSource(workSource, {
		'svelte/store': { writable },
		'#lib/api/work.js': {
			getMyWorkCounts: async () => {
				workCalls++;
				return options.work?.(workCalls) ?? { total: 8, open: 8 };
			}
		}
	});
	const { notificationStore: store } = loadSource(
		notificationSource,
		{
			'#lib/notifications/browser-state.js': {
				claimPresentation: async (id) => {
					if (claims.has(id)) return false;
					claims.add(id);
					return true;
				},
				beginBinding: async () => ({}),
				readBrowserState: async () => ({ identity: ':first:staff' }),
				watchIdentityChanges: () => () => {},
				clearBinding: async () => null
			},
			'#lib/api/notifications.js': {},
			'#lib/stores/auth.js': { authStore },
			'#lib/api/client.js': {
				BACKEND_URL: 'https://school.example',
				getSchoolSubdomainHint: () => null,
				apiClient: {
					post: options.post ?? (async () => ({ success: true })),
					get: async () => {
						notificationCalls++;
						return (
							options.notifications?.(notificationCalls) ?? {
								success: true,
								data: { items: [{ id: 'missed', title: 'Missed' }], unread_count: 12 }
							}
						);
					}
				}
			},
			'#lib/realtime/auth-recovery.js': { realtimeAuthRecovery },
			'#lib/pwa/service-worker-registration.js': {},
			'#lib/realtime/visibility-idle.js': {
				createVisibilityIdle,
				browserVisibilityDependencies: () => ({
					isHidden: () => false,
					setTimer,
					clearTimer,
					addListener() {},
					removeListener() {}
				})
			},
			'#lib/stores/work.js': { workStore },
			'#lib/api/auth.js': {
				authAPI: {
					refreshCurrentUser: async (refreshOptions) => {
						authOptions.push(refreshOptions);
						return options.auth?.(authOptions.length) ?? 'authenticated';
					}
				}
			},
			'svelte-sonner': {
				toast: {
					success(...args) {
						toasts.push(args);
					}
				}
			},
			'svelte/store': { writable, get }
		},
		{
			EventSource: Source,
			document: { hidden: options.hidden ?? false, hasFocus: () => options.focused ?? true },
			setTimeout: setTimer,
			clearTimeout: clearTimer,
			console: {
				log() {},
				error: (...args) => assert.fail(`Unexpected recovery error: ${args.join(' ')}`)
			}
		}
	);
	store.initSSE();
	const source = sources[0];
	return {
		store,
		authStore,
		toasts,
		workStore,
		source,
		sources,
		timers,
		authOptions,
		get notificationCalls() {
			return notificationCalls;
		},
		get workCalls() {
			return workCalls;
		},
		runTimer() {
			assert.equal(timers.size, 1, 'recovery must have only one pending timer');
			const [id, timer] = [...timers][0];
			timers.delete(id);
			timer.callback();
			return timer.delay;
		}
	};
}

test('initial SSE open preserves the existing initial read without duplicate catch-up', async (t) => {
	const h = harness();
	t.after(() => h.store.closeSSE());
	await h.store.fetchNotifications();
	h.source.open();
	await flush();
	assert.equal(h.notificationCalls, 1);
	assert.equal(h.workCalls, 0);
	assert.equal(h.authOptions.length, 0);
	assert.equal(h.timers.size, 0);
});

test('same-identity native reconnect catches up inbox, unread and work after subscribing', async (t) => {
	const h = harness();
	t.after(() => h.store.closeSSE());
	h.source.open();
	h.source.error();
	await flush();
	assert.equal(h.notificationCalls, 0, 'no snapshot while native EventSource is reconnecting');
	assert.equal(h.workCalls, 0);
	assert.equal(h.authOptions.length, 0);
	assert.equal(h.timers.size, 0, 'native reconnect owns connection retry');
	h.source.open();
	await flush();
	assert.equal(h.sources.length, 1, 'native reconnect reuses the same EventSource');
	assert.deepEqual(h.authOptions, [{ silent: true, invalidate: true }]);
	assert.equal(h.notificationCalls, 1);
	assert.equal(h.workCalls, 1);
	assert.equal(get(h.store).notifications[0].id, 'missed');
	assert.equal(get(h.store).unreadCount, 12);
	assert.equal(get(h.workStore).counts.total, 8);
	assert.equal(get(h.workStore).windowsRevision, 1);
	h.source.open();
	await flush();
	assert.equal(h.notificationCalls, 1, 'a clean repeated open does no additional work');
});

test('explicit lag catches up without replacing the subscribed source', async (t) => {
	const h = harness();
	t.after(() => h.store.closeSSE());
	h.source.open();
	h.source.lag();
	await flush();
	assert.equal(h.source.readyState, 1);
	assert.equal(h.sources.length, 1);
	assert.equal(h.notificationCalls, 1);
	assert.equal(h.workCalls, 1);
	assert.equal(get(h.store).unreadCount, 12);
	assert.equal(get(h.workStore).counts.total, 8);
	assert.equal(h.timers.size, 0);
});

test('an initial inbox request from before a gap cannot overwrite completed catch-up', async (t) => {
	const initial = deferred();
	const h = harness({ notifications: (call) => (call === 1 ? initial.promise : undefined) });
	t.after(() => h.store.closeSSE());
	const initialRead = h.store.fetchNotifications();
	h.source.open();
	h.source.error();
	h.source.open();
	await flush();
	assert.equal(get(h.store).unreadCount, 12);
	initial.resolve({ success: true, data: { items: [], unread_count: 99 } });
	assert.equal(await initialRead, false);
	assert.equal(get(h.store).notifications[0].id, 'missed');
	assert.equal(get(h.store).unreadCount, 12);
	assert.equal(h.notificationCalls, 2);
});

test('duplicate open and lag signals coalesce and invalidate a raced snapshot', async (t) => {
	const snapshot = deferred();
	const work = deferred();
	const h = harness({
		notifications: (call) => (call === 1 ? snapshot.promise : undefined),
		work: (call) => (call === 1 ? work.promise : undefined)
	});
	t.after(() => h.store.closeSSE());
	h.source.open();
	h.source.lag();
	await flush();
	h.source.open();
	h.source.lag();
	h.source.lag();
	await flush();
	assert.equal(h.notificationCalls, 1);
	assert.equal(h.workCalls, 1);
	snapshot.resolve({ success: true, data: { items: [], unread_count: 99 } });
	work.resolve({ total: 99 });
	await flush();
	assert.equal(get(h.store).unreadCount, 0, 'a snapshot from before another gap is rejected');
	assert.equal(get(h.workStore).counts.total, 0);
	assert.equal(h.timers.size, 1);
	h.source.open();
	h.source.lag();
	await flush();
	assert.equal(h.notificationCalls, 1, 'duplicate signals respect the coalesced retry delay');
	assert.equal(h.runTimer(), 3000);
	await flush();
	assert.equal(h.notificationCalls, 2);
	assert.equal(h.workCalls, 2);
	assert.equal(get(h.store).unreadCount, 12);
	assert.equal(get(h.workStore).counts.total, 8);
	assert.equal(h.timers.size, 0);
});

test('live events during catch-up cannot be erased by an older inbox snapshot', async (t) => {
	const snapshot = deferred();
	const h = harness({ notifications: (call) => (call === 1 ? snapshot.promise : undefined) });
	t.after(() => h.store.closeSSE());
	h.source.open();
	h.source.lag();
	await flush();
	h.source.onmessage({ data: JSON.stringify({ id: 'live', title: 'Live', message: 'Live' }) });
	snapshot.resolve({ success: true, data: { items: [], unread_count: 99 } });
	await flush();
	assert.equal(get(h.store).notifications[0].id, 'live');
	assert.equal(get(h.store).unreadCount, 1);
	h.runTimer();
	await flush();
	assert.equal(get(h.store).unreadCount, 12);
	assert.equal(h.notificationCalls, 2);
});

test('a native reconnect during catch-up rejects that snapshot and waits for open', async (t) => {
	const snapshot = deferred();
	const h = harness({ notifications: (call) => (call === 1 ? snapshot.promise : undefined) });
	t.after(() => h.store.closeSSE());
	h.source.open();
	h.source.lag();
	await flush();
	h.source.error();
	snapshot.resolve({ success: true, data: { items: [], unread_count: 99 } });
	await flush();
	assert.equal(get(h.store).unreadCount, 0);
	assert.equal(h.timers.size, 0);
	h.source.open();
	await flush();
	assert.equal(get(h.store).unreadCount, 12);
	assert.equal(h.notificationCalls, 2);
});

test('failed catch-up retries with capped backoff and logout cancels the retry', async () => {
	const h = harness({ notifications: () => ({ success: false, error: 'unavailable' }) });
	h.source.open();
	h.source.lag();
	await flush();
	for (const delay of [3000, 6000, 12000, 24000, 48000, 60000, 60000]) {
		assert.equal(h.runTimer(), delay);
		await flush();
	}
	assert.equal(h.notificationCalls, 8);
	const staleRetry = [...h.timers.values()][0].callback;
	h.store.closeSSE();
	assert.equal(h.timers.size, 0);
	staleRetry();
	h.source.open();
	h.source.lag();
	await flush();
	assert.equal(h.notificationCalls, 8);
	assert.equal(h.sources.length, 1);
});

test('a failed work snapshot keeps recovery dirty until both owners catch up', async (t) => {
	const h = harness({
		work: (call) => {
			if (call === 1) throw new Error('work unavailable');
		}
	});
	t.after(() => h.store.closeSSE());
	h.source.open();
	h.source.lag();
	await flush();
	assert.equal(get(h.store).unreadCount, 12);
	assert.equal(get(h.workStore).countsError, 'work unavailable');
	assert.equal(h.runTimer(), 3000);
	await flush();
	assert.equal(h.notificationCalls, 2);
	assert.equal(h.workCalls, 2);
	assert.equal(get(h.workStore).countsError, null);
	assert.equal(get(h.workStore).counts.total, 8);
	assert.equal(h.timers.size, 0);
});

test('unavailable catch-up authentication retries before requesting protected snapshots', async (t) => {
	const h = harness({ auth: (call) => (call === 1 ? 'unavailable' : 'authenticated') });
	t.after(() => h.store.closeSSE());
	h.source.open();
	h.source.lag();
	await flush();
	assert.equal(h.notificationCalls, 0);
	assert.equal(h.workCalls, 0);
	assert.equal(h.runTimer(), 3000);
	await flush();
	assert.equal(h.notificationCalls, 1);
	assert.equal(h.workCalls, 1);
	assert.equal(h.source.readyState, 1);
	assert.equal(h.timers.size, 0);
});

test('a disconnect during catch-up authentication waits for a fresh subscribed snapshot', async (t) => {
	const auth = deferred();
	const h = harness({ auth: (call) => (call === 1 ? auth.promise : 'authenticated') });
	t.after(() => h.store.closeSSE());
	h.source.open();
	h.source.lag();
	await flush();
	h.source.error();
	auth.resolve('authenticated');
	await flush();
	assert.equal(h.notificationCalls, 0);
	assert.equal(h.workCalls, 0);
	assert.equal(h.timers.size, 0);
	h.source.open();
	await flush();
	assert.equal(h.notificationCalls, 1);
	assert.equal(h.workCalls, 1);
});

test('logout during catch-up authentication prevents later snapshot requests', async () => {
	const auth = deferred();
	const h = harness({ auth: () => auth.promise });
	h.source.open();
	h.source.lag();
	await flush();
	h.store.closeSSE();
	auth.resolve('authenticated');
	await flush();
	assert.equal(h.notificationCalls, 0);
	assert.equal(h.workCalls, 0);
	assert.equal(h.timers.size, 0);
});

test('logout rejects pending inbox and work catch-up and stale source callbacks', async () => {
	const snapshot = deferred();
	const work = deferred();
	const h = harness({ notifications: () => snapshot.promise, work: () => work.promise });
	h.source.open();
	h.source.lag();
	await flush();
	h.store.closeSSE();
	snapshot.resolve({ success: true, data: { items: [{ id: 'old-user' }], unread_count: 99 } });
	work.resolve({ total: 99 });
	h.source.onmessage({ data: JSON.stringify({ id: 'old-event', title: 'Old' }) });
	h.source.events.permission_changed();
	h.source.events.work_items_changed();
	await flush();
	assert.deepEqual(get(h.store).notifications, []);
	assert.equal(get(h.store).unreadCount, 0);
	assert.equal(get(h.workStore).counts.total, 0);
	assert.equal(h.authOptions.length, 1);
	assert.equal(h.workCalls, 1);
	assert.equal(h.timers.size, 0);
});

test('session recovery validates identity, then subscribes before authoritative catch-up', async (t) => {
	const h = harness();
	t.after(() => h.store.closeSSE());
	h.source.open();
	h.source.events.session_invalid();
	await flush();
	assert.equal(h.source.readyState, 2);
	assert.deepEqual(h.authOptions, [{ silent: true }]);
	assert.equal(h.notificationCalls, 0);
	h.runTimer();
	assert.equal(h.sources.length, 2);
	assert.equal(h.notificationCalls, 0);
	h.sources[1].open();
	await flush();
	assert.deepEqual(h.authOptions[1], { silent: true, invalidate: true });
	assert.equal(h.notificationCalls, 1);
	assert.equal(h.workCalls, 1);
});

test('authoritative unauthenticated recovery closes the stream without catch-up or retries', async () => {
	const h = harness({ auth: () => 'unauthenticated' });
	h.source.open();
	h.source.error(2);
	await flush();
	assert.equal(h.source.readyState, 2);
	assert.equal(h.notificationCalls, 0);
	assert.equal(h.workCalls, 0);
	assert.equal(h.timers.size, 0);
});

function live(id, extra = {}) {
	return {
		id,
		title: id,
		message: id,
		created_at: new Date().toISOString(),
		read_at: null,
		...extra
	};
}
function send(h, notification) {
	h.source.onmessage({ data: JSON.stringify(notification) });
}

test('duplicate live events present and count once even after the latest inbox evicts them', async (t) => {
	const h = harness();
	t.after(() => h.store.closeSSE());
	h.source.open();
	const first = live('first');
	send(h, first);
	send(h, first);
	await flush();
	assert.equal(get(h.store).unreadCount, 1);
	assert.equal(h.toasts.length, 1);
	for (let index = 0; index < 12; index++) send(h, live(`next-${index}`));
	await flush();
	assert.equal(get(h.store).notifications.length, 10);
	const count = get(h.store).unreadCount;
	send(h, first);
	await flush();
	assert.equal(get(h.store).unreadCount, count);
	assert.equal(h.toasts.length, 13);
});
test('authoritative snapshots are silent and preserve a separate read ledger and total unread', async (t) => {
	let items = [live('read-me')];
	const h = harness({
		notifications: () => ({ success: true, data: { items, unread_count: 42 } })
	});
	t.after(() => h.store.closeSSE());
	h.source.open();
	await h.store.fetchNotifications();
	assert.equal(h.toasts.length, 0);
	await h.store.markAsRead('read-me');
	assert.equal(get(h.store).unreadCount, 41, 'one read subtracts one from the full server total');
	await h.store.markAsRead('read-me');
	assert.equal(get(h.store).unreadCount, 41);
	items = [live('newest')];
	await h.store.fetchNotifications();
	send(h, live('read-me'));
	await flush();
	assert.equal(
		get(h.store).unreadCount,
		42,
		'read replay outside recent page does not add a count'
	);
	assert.equal(h.toasts.length, 0);
});
test('read, expired and snapshot-older events never create fresh presentation or unread counts', async (t) => {
	const now = Date.now();
	const h = harness({
		notifications: () => ({
			success: true,
			data: {
				items: [live('snapshot', { created_at: new Date(now).toISOString() })],
				unread_count: 20
			}
		})
	});
	t.after(() => h.store.closeSSE());
	h.source.open();
	await h.store.fetchNotifications();
	send(h, live('older', { created_at: new Date(now - 1000).toISOString() }));
	send(h, live('already-read', { read_at: new Date().toISOString() }));
	send(h, live('expired', { created_at: new Date(now - 3600_000).toISOString() }));
	await flush();
	assert.equal(h.toasts.length, 0);
	assert.equal(get(h.store).unreadCount, 20);
});
test('foreground eligibility and atomic installation claims suppress duplicate tab presentation', async (t) => {
	const claims = new Set();
	const a = harness({ claims });
	const b = harness({ claims });
	const hidden = harness({ hidden: true });
	t.after(() => {
		a.store.closeSSE();
		b.store.closeSSE();
		hidden.store.closeSSE();
	});
	for (const h of [a, b, hidden]) {
		h.source.open();
		send(h, live('shared'));
	}
	await flush();
	assert.equal(a.toasts.length + b.toasts.length, 1);
	assert.equal(hidden.toasts.length, 0);
	assert.equal(get(a.store).unreadCount, 1);
	assert.equal(get(b.store).unreadCount, 1);
});
test('account binding clears identity data and rejects old source, pending read and mutation callbacks', async () => {
	const snapshot = deferred();
	const mutation = deferred();
	const h = harness({ notifications: () => snapshot.promise, post: () => mutation.promise });
	h.store.bindIdentity();
	h.store.initSSE();
	const source = h.sources.at(-1);
	source.open();
	source.onmessage({ data: JSON.stringify(live('old-account')) });
	await flush();
	const read = h.store.fetchNotifications();
	const mark = h.store.markAsRead('old-account');
	h.authStore.sessionEpoch++;
	h.authStore.set({ isAuthenticated: true, user: { id: 'second', user_type: 'staff' } });
	h.store.bindIdentity();
	snapshot.resolve({ success: true, data: { items: [live('old-snapshot')], unread_count: 99 } });
	mutation.resolve({ success: true });
	await Promise.all([read, mark]);
	source.onmessage({ data: JSON.stringify(live('old-callback')) });
	await flush();
	assert.deepEqual(get(h.store), { notifications: [], unreadCount: 0, loading: false });
	h.store.closeSSE();
});
