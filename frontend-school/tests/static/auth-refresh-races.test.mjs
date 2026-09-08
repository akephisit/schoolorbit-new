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
async function harness() {
	const { authStore } = await load('../../src/lib/stores/auth.ts', {
		'svelte/store': { writable },
		'./permissions': { setPermissions() {}, clearPermissions() {} }
	});
	let state;
	authStore.subscribe((value) => (state = value));
	const requests = [];
	const { authAPI } = await load('../../src/lib/api/auth.ts', {
		'$lib/api/client': {
			apiClient: { get: () => new Promise((resolve) => requests.push(resolve)) }
		},
		'$lib/api/session-security': { clearSessionSecurity() {} },
		'$lib/auth/auth-refresh-policy': { authRefreshDecision },
		'$lib/stores/auth': { authStore },
		'svelte-sonner': { toast: {} }
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

test('work counts and items ignore older overlapping responses', async () => {
	const counts = [];
	const items = [];
	const { workStore } = await load('../../src/lib/stores/work.ts', {
		'svelte/store': { writable },
		'$lib/api/work': {
			getMyWorkCounts: () => new Promise((resolve) => counts.push(resolve)),
			getMyWorkItems: () => new Promise((resolve) => items.push(resolve))
		}
	});
	let state;
	workStore.subscribe((value) => (state = value));
	const old = workStore.refreshSilently();
	const fresh = workStore.refreshSilently();
	counts[1]({ total: 2 });
	items[1]([{ id: 'fresh' }]);
	await fresh;
	counts[0]({ total: 1 });
	items[0]([{ id: 'old' }]);
	await old;
	assert.equal(state.counts.total, 2);
	assert.equal(state.items[0].id, 'fresh');
	const pending = workStore.refreshSilently();
	workStore.reset();
	counts[2]({ total: 3 });
	items[2]([{ id: 'logout' }]);
	await pending;
	assert.equal(state.counts.total, 0);
	assert.deepEqual(state.items, []);
});
