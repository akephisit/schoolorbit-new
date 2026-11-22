import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';
import { get, writable } from 'svelte/store';
import { authRefreshDecision } from '../../src/lib/auth/auth-refresh-policy.ts';

async function load(relative, dependencies) {
	const source = await readFile(new URL(relative, import.meta.url), 'utf8');
	const compiled = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
	}).outputText;
	const exports = {};
	new Function('require', 'exports', compiled)((id) => {
		assert.ok(id in dependencies, id);
		return dependencies[id];
	}, exports);
	return exports;
}
async function harness(extraClient = {}, notifications = { async prepareLogout() {} }) {
	const { authStore } = await load('../../src/lib/stores/auth.ts', {
		'svelte/store': { writable },
		'./permissions': { setPermissions() {}, clearPermissions() {} }
	});
	let state;
	authStore.subscribe((value) => (state = value));
	const requests = [];
	const { authAPI } = await load('../../src/lib/api/auth.ts', {
		'svelte/store': { get },
		'#lib/api/client.js': {
			ApiClientError: class extends Error {
				constructor(message, status) {
					super(message);
					this.status = status;
				}
			},
			apiClient: { get: () => new Promise((resolve) => requests.push(resolve)), ...extraClient },
			requireApiData(response) {
				if (!response.success) throw new Error(response.error || 'request failed');
				return response.data;
			}
		},
		'#lib/api/session-security.js': { clearSessionSecurity() {} },
		'#lib/auth/auth-refresh-policy.js': { authRefreshDecision },
		'#lib/auth/session-awareness.js': { announceAuthenticatedSession() {} },
		'#lib/stores/auth.js': { authStore },
		'#lib/stores/notification.js': { notificationStore: notifications },
		'svelte-sonner': { toast: { success() {}, error() {} } }
	});
	return {
		authAPI,
		authStore,
		requests,
		get state() {
			return state;
		}
	};
}
const response = {
	status: 200,
	success: true,
	data: {
		id: 'old',
		firstName: 'Old',
		lastName: 'User',
		userType: 'staff',
		status: 'active',
		session: {
			sessionId: '01900000-0000-7000-8000-000000000001',
			serverNow: '2026-10-29T00:00:00Z',
			idleExpiresAt: '2026-10-29T02:00:00Z',
			absoluteExpiresAt: '2026-10-29T12:00:00Z',
			nextRenewalAt: '2026-10-29T00:05:00Z'
		},
		permissions: []
	}
};
const expired = { status: 401, success: false, error: 'expired' };
const flush = () => new Promise((resolve) => setImmediate(resolve));
const restored = (user = response.data) => ({ status: 200, success: true, data: { user } });

test('current-user 401 restores once with deliberate empty JSON and shares the whole refresh', async () => {
	const writes = [];
	let release;
	const h = await harness({
		post: (endpoint, body) => {
			writes.push({ endpoint, body });
			return new Promise((resolve) => (release = resolve));
		}
	});
	const first = h.authAPI.refreshCurrentUser();
	h.requests[0](expired);
	await flush();
	const concurrent = h.authAPI.refreshCurrentUser();
	assert.deepEqual(writes, [{ endpoint: '/api/auth/restore', body: {} }]);
	release(restored());
	assert.deepEqual(await Promise.all([first, concurrent]), ['authenticated', 'authenticated']);
	assert.equal(h.requests.length, 1);
	assert.equal(h.state.user.id, 'old');
});

for (const [status, expected] of [
	[401, 'unauthenticated'],
	[503, 'unavailable']
]) {
	test(`restore ${status} has an explicit ${expected} outcome`, async () => {
		const h = await harness({ post: async () => ({ status, success: false, error: 'refused' }) });
		h.authStore.setUser({ id: 'old', user_type: 'staff' }, [], response.data.session);
		const pending = h.authAPI.refreshCurrentUser();
		h.requests[0](expired);
		assert.equal(await pending, expected);
		assert.equal(h.state.isAuthenticated, status === 503);
		assert.equal(h.state.isUnavailable, status === 503);
	});
}

test('restore 409 re-reads current user once and never restores again in that refresh', async () => {
	for (const current of [response, expired]) {
		let attempts = 0;
		const h = await harness({
			post: async () => {
				attempts++;
				return { status: 409, success: false, error: 'session_exists' };
			}
		});
		const pending = h.authAPI.refreshCurrentUser();
		h.requests[0](expired);
		await flush();
		assert.equal(h.requests.length, 2);
		h.requests[1](current);
		assert.equal(await pending, current === response ? 'authenticated' : 'unauthenticated');
		assert.equal(attempts, 1);
	}
});

test('explicit login fences delayed restore state and runs after its cookie response', async () => {
	const writes = [];
	const h = await harness({
		post: (endpoint) => new Promise((resolve) => writes.push({ endpoint, resolve }))
	});
	const refresh = h.authAPI.refreshCurrentUser();
	h.requests[0](expired);
	await flush();
	const login = h.authAPI.login({ username: 'new', password: 'fixture', rememberMe: true });
	assert.deepEqual(
		writes.map((item) => item.endpoint),
		['/api/auth/restore']
	);
	writes[0].resolve(restored());
	assert.equal(await refresh, 'unauthenticated');
	await flush();
	assert.equal(h.state.user, null);
	assert.deepEqual(
		writes.map((item) => item.endpoint),
		['/api/auth/restore', '/api/auth/login']
	);
	writes[1].resolve(restored({ ...response.data, id: 'new' }));
	assert.equal((await login).id, 'new');
	assert.equal(h.state.user.id, 'new');
});

test('login preserves the authoritative rejection after generic transport 401 cleanup changes epoch', async () => {
	let h;
	h = await harness({
		post: async () => {
			h.authStore.clearUser();
			return { status: 401, success: false, error: 'incorrect credentials' };
		}
	});
	await assert.rejects(
		h.authAPI.login({ username: 'fixture', password: 'fixture', rememberMe: false }),
		/incorrect credentials/
	);
	assert.equal(h.state.user, null);
	assert.equal(h.state.isLoading, false);
});

for (const status of [200, 401, 503]) {
	test(`a delayed restore ${status} cannot replace or erase a newer observed login`, async () => {
		let release;
		const h = await harness({ post: () => new Promise((resolve) => (release = resolve)) });
		const pending = h.authAPI.refreshCurrentUser();
		h.requests[0](expired);
		await flush();
		h.authStore.setUser({ id: 'new', user_type: 'staff' }, [], {
			...response.data.session,
			sessionId: 'new-session'
		});
		release(status === 200 ? restored() : { status, success: false, error: 'refused' });
		assert.equal(await pending, 'authenticated', 'stale callers must not redirect a newer login');
		assert.equal(h.state.user.id, 'new');
		assert.equal(h.state.isUnavailable, false);
	});
}

test('explicit logout waits for restore cookies and revokes even with an expired child session', async () => {
	const writes = [];
	const h = await harness({
		post: (endpoint) => new Promise((resolve) => writes.push({ endpoint, resolve }))
	});
	h.authStore.setUser({ id: 'old', user_type: 'staff' }, [], response.data.session);
	const refresh = h.authAPI.refreshCurrentUser();
	h.requests[0](expired);
	await flush();
	const logout = h.authAPI.logout();
	writes[0].resolve(restored());
	await refresh;
	await flush();
	assert.equal(h.requests.length, 2);
	h.requests[1](expired);
	await flush();
	assert.deepEqual(
		writes.map((item) => item.endpoint),
		['/api/auth/restore', '/api/auth/logout']
	);
	writes[1].resolve({ status: 200, success: true, data: {} });
	await logout;
	assert.equal(h.state.user, null);
});

test('expired-session logout performs browser cleanup and explicit grant revocation without restore', async () => {
	const posts = [];
	let cleanups = 0;
	const h = await harness(
		{
			get: async () => expired,
			post: async (endpoint) => {
				posts.push(endpoint);
				return { status: 200, success: true, data: {} };
			}
		},
		{
			async prepareLogout() {
				cleanups++;
			}
		}
	);
	h.authStore.setUser({ id: 'old', user_type: 'staff' }, [], response.data.session);
	await h.authAPI.logout();
	assert.deepEqual(posts, ['/api/auth/logout']);
	assert.equal(cleanups, 1);
	assert.equal(h.state.isAuthenticated, false);
});

test('revoke refuses a different current session or account without issuing the selected mutation', async () => {
	for (const current of [
		{ ...response.data, id: 'another' },
		{ ...response.data, session: { ...response.data.session, sessionId: 'another-session' } }
	]) {
		let deletes = 0;
		const h = await harness({
			get: async () => ({ ...response, data: current }),
			delete: async () => {
				deletes++;
				return { success: true, data: {} };
			}
		});
		h.authStore.setUser({ id: 'old', user_type: 'staff' }, [], response.data.session);
		await assert.rejects(h.authAPI.revokeSession('selected', { current: true }), /เปลี่ยนแปลง/);
		assert.equal(deletes, 0);
		assert.equal(h.state.user.id, 'old');
	}
});

test('denied revoke preserves the authenticated user and is never retried', async () => {
	let deletes = 0;
	const h = await harness({
		get: async () => response,
		delete: async () => {
			deletes++;
			return { status: 403, success: false, error: 'forbidden' };
		}
	});
	h.authStore.setUser({ id: 'old', user_type: 'staff' }, [], response.data.session);
	await assert.rejects(h.authAPI.revokeSession('selected', { current: true }), /forbidden/);
	assert.equal(deletes, 1);
	assert.equal(h.state.user.id, 'old');
});
test('concurrent current-user refreshes share one request', async () => {
	const h = await harness();
	const a = h.authAPI.refreshCurrentUser();
	const b = h.authAPI.refreshCurrentUser();
	assert.equal(h.requests.length, 1);
	h.requests[0](response);
	await Promise.all([a, b]);
	assert.equal(h.state.user.id, 'old');
});
test('refresh completion cannot restore a logged-out user', async () => {
	const h = await harness();
	const pending = h.authAPI.refreshCurrentUser();
	h.authStore.clearUser();
	h.requests[0](response);
	assert.equal(await pending, 'unauthenticated');
	assert.equal(h.state.user, null);
});
test('account change starts a fresh request and ignores the older result', async () => {
	const h = await harness();
	const old = h.authAPI.refreshCurrentUser();
	h.authStore.setUser({ id: 'new' }, []);
	const fresh = h.authAPI.refreshCurrentUser();
	assert.equal(h.requests.length, 2);
	h.requests[0](response);
	await old;
	assert.equal(h.state.user.id, 'new');
	h.requests[1]({ ...response, data: { ...response.data, id: 'new' } });
	await fresh;
	assert.equal(h.state.user.id, 'new');
});

test('default refresh stays silent while a visible caller can share the request', async () => {
	const h = await harness();
	h.authStore.setUser({ id: 'old' }, []);
	const silent = h.authAPI.refreshCurrentUser();
	assert.equal(h.state.isLoading, false);
	const visible = h.authAPI.refreshCurrentUser({ silent: false });
	assert.equal(h.state.isLoading, true);
	assert.equal(h.requests.length, 1);
	h.requests[0](response);
	await Promise.all([silent, visible]);
	assert.equal(h.state.isLoading, false);
});

test('permission signal during refresh discards old snapshot and queues one fresh read', async () => {
	const h = await harness();
	h.authStore.setUser({ id: 'old' }, []);
	const first = h.authAPI.refreshCurrentUser();
	const signal = h.authAPI.refreshCurrentUser({ silent: true, invalidate: true });
	h.requests[0](response);
	await new Promise((resolve) => setImmediate(resolve));
	assert.equal(h.requests.length, 2);
	assert.equal(h.state.user.firstName, undefined, 'pre-signal snapshot must not apply');
	h.requests[1]({ ...response, data: { ...response.data, firstName: 'Updated' } });
	assert.deepEqual(await Promise.all([first, signal]), ['authenticated', 'authenticated']);
	assert.equal(h.state.user.firstName, 'Updated');
});

test('work changes signal active route reconciliation without fetching hidden items', async () => {
	const counts = [];
	let itemReads = 0;
	const { workStore } = await load('../../src/lib/stores/work.ts', {
		'svelte/store': { writable },
		'#lib/api/work.js': {
			getMyWorkCounts: () => new Promise((resolve) => counts.push(resolve)),
			getMyWorkItems: () => {
				itemReads++;
				return Promise.resolve([]);
			}
		}
	});
	let state;
	workStore.subscribe((value) => (state = value));
	const old = workStore.refreshSilently();
	const fresh = workStore.refreshSilently();
	assert.equal(state.revision, 2);
	assert.equal(state.windowsRevision, 0);
	assert.equal(itemReads, 0);
	counts[1]({ total: 2 });
	await fresh;
	counts[0]({ total: 1 });
	await old;
	assert.equal(state.counts.total, 2);
	assert.equal(state.revision, 2);
	const pending = workStore.refreshSilently({ windowsChanged: true });
	assert.equal(state.windowsRevision, 1);
	workStore.reset();
	counts[2]({ total: 3 });
	await pending;
	assert.equal(state.counts.total, 0);
	assert.equal(state.revision, 0);
	assert.equal(state.windowsRevision, 0);
	await workStore.refreshSilently({ isCurrent: () => false });
	assert.equal(counts.length, 3);
	assert.equal(state.revision, 0);
});

test('layout-owned counts cannot overwrite a newer refresh or refill a reset identity', async () => {
	const counts = [];
	const { workStore } = await load('../../src/lib/stores/work.ts', {
		'svelte/store': { writable },
		'#lib/api/work.js': {
			getMyWorkCounts: () => new Promise((resolve) => counts.push(resolve)),
			getMyWorkItems: async () => []
		}
	});
	let state;
	workStore.subscribe((value) => (state = value));
	let release;
	workStore.consumeRouteCounts(new Promise((resolve) => (release = resolve)), () => true);
	const refresh = workStore.fetchCounts();
	counts[0]({ total: 2 });
	await refresh;
	release({ ok: true, data: { identityKey: 'old', counts: { total: 1 } }, error: null });
	await Promise.resolve();
	assert.equal(state.counts.total, 2);
	workStore.consumeRouteCounts(new Promise((resolve) => (release = resolve)), () => true);
	workStore.reset();
	release({ ok: true, data: { identityKey: 'old', counts: { total: 3 } }, error: null });
	await Promise.resolve();
	assert.equal(state.counts.total, 0);
	assert.equal(state.loadedCounts, false);
	workStore.consumeRouteCounts(
		Promise.resolve({
			ok: true,
			data: { identityKey: 'another-user', counts: { total: 4 } },
			error: null
		}),
		() => false
	);
	await Promise.resolve();
	assert.equal(state.counts.total, 0);
});

test('route session identity survives refresh but changes across same-user reauthentication', async () => {
	const h = await harness();
	const user = { id: 'same', firstName: 'Same', lastName: 'User', user_type: 'staff' };
	h.authStore.setUser(user, []);
	const before = h.authStore.sessionEpoch;
	assert.equal(typeof before, 'number');
	h.authStore.setUser(user, []);
	assert.equal(h.authStore.sessionEpoch, before);
	h.authStore.clearUser();
	h.authStore.setUser(user, []);
	assert.notEqual(h.authStore.sessionEpoch, before);
});

test('activity metadata updates preserve auth/route generation and reject stale snapshots', async () => {
	const h = await harness();
	const user = { id: 'same', user_type: 'staff' };
	const session = response.data.session;
	h.authStore.setUser(user, [], session);
	const epoch = h.authStore.sessionEpoch;
	const generation = h.authStore.generation;
	const renewed = {
		...session,
		serverNow: '2026-10-29T00:05:00Z',
		nextRenewalAt: '2026-10-29T00:10:00Z'
	};
	h.authStore.updateSession(renewed, epoch);
	assert.equal(h.state.session, renewed);
	assert.equal(h.authStore.sessionEpoch, epoch);
	assert.equal(h.authStore.generation, generation);
	h.authStore.updateSession(session, epoch);
	assert.equal(h.state.session, renewed);
	h.authStore.updateSession({ ...renewed, sessionId: 'another' }, epoch);
	assert.equal(h.state.session, renewed);
});

test('same-user session replacement without logout fences an older current-user response', async () => {
	const h = await harness();
	const user = { id: 'same', user_type: 'staff' };
	h.authStore.setUser(user, [], response.data.session);
	const before = h.authStore.sessionEpoch;
	const pending = h.authAPI.refreshCurrentUser();
	const replacement = {
		...response.data.session,
		sessionId: '01900000-0000-7000-8000-000000000002'
	};
	h.authStore.setUser(user, [], replacement);
	assert.notEqual(h.authStore.sessionEpoch, before);
	h.requests[0](response);
	await pending;
	assert.equal(h.state.session, replacement);
});

test('missing required session metadata is unavailable and cannot establish healthy authentication', async () => {
	const h = await harness();
	const pending = h.authAPI.refreshCurrentUser();
	h.requests[0]({ ...response, data: { ...response.data, session: undefined } });
	assert.equal(await pending, 'unavailable');
	assert.equal(h.state.isAuthenticated, false);
});

for (const action of ['logout', 'logoutAll', 'revokeSession']) {
	test(`${action}: late completion does not clear a newer authenticated session`, async () => {
		let release;
		const operation = () =>
			new Promise((resolve) => {
				release = resolve;
			});
		const h = await harness({
			get: async () => ({ ...response, data: { ...response.data, id: 'same' } }),
			post: operation,
			delete: operation
		});
		const user = { id: 'same', firstName: 'Same', lastName: 'User', user_type: 'staff' };
		h.authStore.setUser(user, [], response.data.session);
		const pending =
			action === 'revokeSession'
				? h.authAPI.revokeSession('current', { current: true })
				: h.authAPI[action]();
		await new Promise((resolve) => setImmediate(resolve));
		h.authStore.clearUser();
		h.authStore.setUser(user, []);
		release({ success: true, data: {} });
		await pending;
		assert.equal(h.state.user.id, 'same');
	});
	test(`${action}: an ordinary current-user refresh still permits session cleanup`, async () => {
		let release;
		const operation = () =>
			new Promise((resolve) => {
				release = resolve;
			});
		const h = await harness({
			get: async () => ({ ...response, data: { ...response.data, id: 'same' } }),
			post: operation,
			delete: operation
		});
		const user = { id: 'same', firstName: 'Same', lastName: 'User', user_type: 'staff' };
		h.authStore.setUser(user, [], response.data.session);
		const pending =
			action === 'revokeSession'
				? h.authAPI.revokeSession('current', { current: true })
				: h.authAPI[action]();
		await new Promise((resolve) => setImmediate(resolve));
		h.authStore.setUser(user, [], response.data.session);
		release({ success: true, data: {} });
		await pending;
		assert.equal(h.state.user, null);
	});
}

async function transportHarness(securityOverrides = {}) {
	const h = await harness();
	const captured = [];
	const { apiClient } = await load('../../src/lib/api/client.ts', {
		'$app/env': { browser: false },
		'$app/paths': { resolve: (value) => value },
		'$app/env/public': {
			PUBLIC_BACKEND_URL: 'https://api.example.invalid',
			PUBLIC_SCHOOL_SUBDOMAIN: ''
		},
		'#lib/api/session-security.js': {
			captureSessionSecurityHeaders(headers) {
				captured.push(headers.get('X-CSRF-Token'));
			},
			clearSessionSecurity() {},
			retryAfterSeconds() {},
			withSessionSecurityHeaders: (_method, headers) => headers,
			...securityOverrides
		},
		'#lib/api/query.js': { appendApiQuery: (endpoint) => endpoint },
		'#lib/api/school-subdomain.js': { normalizeSchoolSubdomain: (value) => value ?? null },
		'#lib/deployment/maintenance.js': { confirmMaintenance() {}, probeDeploymentStatus() {} },
		'#lib/deployment/maintenance-controller.js': { isMaintenanceResponse: () => false },
		'#lib/stores/auth.js': { authStore: h.authStore }
	});
	return {
		...h,
		apiClient,
		captured,
		get state() {
			return h.state;
		}
	};
}

test('JSON request headers carry lease proof without replacing session, tenant or content defaults', async () => {
	const security = await import('../../src/lib/api/session-security.ts');
	security.captureSessionSecurityHeaders(new Headers({ 'X-CSRF-Token': 'authoritative-csrf' }));
	const h = await transportHarness(security);
	const caller = new Headers({
		'X-Timetable-Connection-Id': 'connection-one',
		'X-Timetable-Lease-Token': 'lease-one',
		'X-Timetable-Mutation-Id': 'mutation-one',
		'X-CSRF-Token': 'untrusted-csrf',
		'X-School-Subdomain': 'untrusted-school',
		'Content-Type': 'text/plain'
	});
	const captured = [];
	const options = {
		headers: caller,
		requestFetch: async (_url, init) => {
			captured.push(init);
			return new Response(JSON.stringify({ success: true, data: {} }), {
				headers: { 'Content-Type': 'application/json' }
			});
		}
	};
	await h.apiClient.get('/api/example', options);
	await h.apiClient.post('/api/example', { title: 'example' }, options);
	await h.apiClient.put('/api/example', {}, options);
	await h.apiClient.patch('/api/example', {}, options);
	await h.apiClient.delete('/api/example', options);
	await h.apiClient.deleteWithBody('/api/example', {}, options);
	for (const request of captured) {
		assert.equal(request.credentials, 'include');
		assert.equal(request.headers.get('X-Timetable-Connection-Id'), 'connection-one');
		assert.equal(request.headers.get('X-Timetable-Lease-Token'), 'lease-one');
		assert.equal(request.headers.get('X-Timetable-Mutation-Id'), 'mutation-one');
		assert.equal(request.headers.has('X-School-Subdomain'), false);
		assert.equal(
			request.headers.get('X-CSRF-Token'),
			request.method === 'GET' ? null : 'authoritative-csrf'
		);
		if (request.body !== undefined)
			assert.equal(request.headers.get('Content-Type'), 'application/json');
	}
	assert.equal(caller.get('X-CSRF-Token'), 'untrusted-csrf', 'caller headers stay immutable');
	security.clearSessionSecurity();
});
for (const status of [200, 401]) {
	test(`late ${status} cannot change CSRF or identity after same-user reauthentication`, async () => {
		const h = await transportHarness();
		const user = { id: 'same', firstName: 'Same', lastName: 'User', user_type: 'staff' };
		h.authStore.setUser(user, []);
		let release;
		const pending = h.apiClient.get('/api/held', {
			requestFetch: () =>
				new Promise((resolve) => {
					release = resolve;
				})
		});
		h.authStore.clearUser();
		h.authStore.setUser(user, []);
		release(
			new Response(
				JSON.stringify(
					status === 200 ? { success: true, data: [] } : { success: false, error: 'old session' }
				),
				{
					status,
					headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': 'old-synthetic-csrf' }
				}
			)
		);
		const result = await pending;
		assert.equal(result.status, status);
		assert.equal(h.state.user?.id, 'same');
		assert.deepEqual(h.captured, []);
	});
}
test('a current-session 401 still clears authentication after an ordinary refresh', async () => {
	const h = await transportHarness();
	const user = { id: 'same', firstName: 'Same', lastName: 'User', user_type: 'staff' };
	h.authStore.setUser(user, []);
	let release;
	const pending = h.apiClient.get('/api/held', {
		requestFetch: () =>
			new Promise((resolve) => {
				release = resolve;
			})
	});
	h.authStore.setUser(user, []);
	release(
		new Response(JSON.stringify({ success: false, error: 'expired' }), {
			status: 401,
			headers: { 'Content-Type': 'application/json' }
		})
	);
	await pending;
	assert.equal(h.state.user, null);
});

for (const endpoint of ['/api/auth/me', '/api/auth/restore']) {
	test(`${endpoint}: controlled 401 preserves identity until AuthAPI decides the outcome`, async () => {
		const h = await transportHarness();
		h.authStore.setUser({ id: 'same' }, []);
		let request;
		const options = {
			requestFetch: async (_url, init) => {
				request = init;
				return new Response(JSON.stringify(expired), {
					status: 401,
					headers: { 'Content-Type': 'application/json' }
				});
			}
		};
		if (endpoint.endsWith('/restore')) await h.apiClient.post(endpoint, {}, options);
		else await h.apiClient.get(endpoint, options);
		assert.equal(h.state.user.id, 'same');
		assert.equal(request.credentials, 'include');
		if (endpoint.endsWith('/restore')) {
			assert.equal(request.body, '{}');
			assert.equal(request.headers.get('Content-Type'), 'application/json');
		}
	});
}

for (const status of [200, 401]) {
	test(`a delayed restore ${status} cannot capture CSRF after explicit auth intent starts`, async () => {
		const h = await transportHarness();
		h.authStore.setUser({ id: 'same' }, []);
		const epoch = h.authStore.sessionEpoch;
		let release;
		const pending = h.apiClient.post(
			'/api/auth/restore',
			{},
			{
				requestFetch: () => new Promise((resolve) => (release = resolve))
			}
		);
		h.authStore.beginAuthChange();
		assert.equal(
			h.authStore.sessionEpoch,
			epoch,
			'intent fences before the selected session changes'
		);
		release(
			new Response(JSON.stringify(status === 200 ? restored() : expired), {
				status,
				headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': 'stale-restore-csrf' }
			})
		);
		await pending;
		assert.deepEqual(h.captured, []);
		assert.equal(h.state.user.id, 'same');
	});
}
