import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';
import { writable } from 'svelte/store';
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
async function harness(extraClient = {}) {
	const { authStore } = await load('../../src/lib/stores/auth.ts', {
		'svelte/store': { writable },
		'./permissions': { setPermissions() {}, clearPermissions() {} }
	});
	let state;
	authStore.subscribe((value) => (state = value));
	const requests = [];
	const { authAPI } = await load('../../src/lib/api/auth.ts', {
		'#lib/api/client.js': {
			apiClient: { get: () => new Promise((resolve) => requests.push(resolve)), ...extraClient },
			requireApiData(response) {
				assert.equal(response.success, true);
				return response.data;
			}
		},
		'#lib/api/session-security.js': { clearSessionSecurity() {} },
		'#lib/auth/auth-refresh-policy.js': { authRefreshDecision },
		'#lib/stores/auth.js': { authStore },
		'svelte-sonner': { toast: { success() {} } }
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
		permissions: []
	}
};
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

for (const action of ['logout', 'logoutAll', 'revokeSession']) {
	test(`${action}: late completion does not clear a newer authenticated session`, async () => {
		let release;
		const operation = () =>
			new Promise((resolve) => {
				release = resolve;
			});
		const h = await harness({ post: operation, delete: operation });
		const user = { id: 'same', firstName: 'Same', lastName: 'User', user_type: 'staff' };
		h.authStore.setUser(user, []);
		const pending =
			action === 'revokeSession'
				? h.authAPI.revokeSession('current', { current: true })
				: h.authAPI[action]();
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
		const h = await harness({ post: operation, delete: operation });
		const user = { id: 'same', firstName: 'Same', lastName: 'User', user_type: 'staff' };
		h.authStore.setUser(user, []);
		const pending =
			action === 'revokeSession'
				? h.authAPI.revokeSession('current', { current: true })
				: h.authAPI[action]();
		h.authStore.setUser(user, []);
		release({ success: true, data: {} });
		await pending;
		assert.equal(h.state.user, null);
	});
}

async function transportHarness() {
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
			withSessionSecurityHeaders: (_method, headers) => headers
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
