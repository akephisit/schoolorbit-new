import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';
import { writable, get } from 'svelte/store';
import { realtimeAuthRecovery } from '../../src/lib/realtime/auth-recovery.ts';
import { createDraftCollaborationSession } from '../../src/lib/academic/timetable/collaboration-session.ts';
import { decodeDraftEvent } from '../../src/lib/academic/timetable/collaboration-decoder.ts';
import { collaborationWorkspace } from '../runtime/timetable-collaboration-fixture.ts';
import { timetableIds } from '../e2e/timetable-test-harness.ts';
import { sessionState } from '../e2e/fixtures/session-state.ts';

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
const params = { academicTermId: 'term-a', currentUserId: 'user-a', timetableVersionId: 'draft-a' };
const user = {
	id: 'user-a',
	user_type: 'staff',
	displayName: 'คนเดิม',
	firstName: 'คน',
	lastName: 'เดิม',
	role: 'staff',
	status: 'active'
};
async function harness(refresh = async () => 'authenticated') {
	const { authStore } = await load('../../src/lib/stores/auth.ts', {
		'svelte/store': { writable },
		'./permissions': { setPermissions() {}, clearPermissions() {} }
	});
	authStore.setUser(user, ['*'], sessionState());
	let runtime;
	let refreshes = 0;
	let confirmations = 0;
	const store = await load('../../src/lib/stores/timetable-socket.ts', {
		'svelte/store': { writable, get },
		'#lib/stores/auth.js': { authStore },
		'#lib/realtime/visibility-idle.js': { browserVisibilityDependencies: () => ({}) },
		'#lib/api/client.js': {
			BACKEND_WS_URL: 'wss://school.example',
			getSchoolSubdomainHint: () => null
		},
		'#lib/realtime/auth-recovery.js': { realtimeAuthRecovery },
		'#lib/academic/timetable/collaboration-session.js': { createDraftCollaborationSession },
		'#lib/academic/timetable/collaboration-decoder.js': { decodeDraftEvent },
		'#lib/utils/timetable-socket-runtime.js': {
			createTimetableSocketRuntime(config) {
				runtime = config;
				return {
					connect() {},
					disconnect() {},
					confirmStateSync() {
						confirmations++;
					},
					send() {
						return true;
					}
				};
			}
		},
		'#lib/api/auth.js': {
			authAPI: {
				refreshCurrentUser: async () => {
					refreshes++;
					return refresh(authStore);
				}
			}
		}
	});
	store.connectTimetableSocket(params);
	return {
		authStore,
		store,
		get runtime() {
			return runtime;
		},
		get confirmations() {
			return confirmations;
		},
		get refreshes() {
			return refreshes;
		}
	};
}

test('policy refresh accepts same-session metadata renewal and propagates public auth outcomes', async () => {
	const stable = await harness(async (authStore) => {
		const current = get(authStore);
		authStore.setUser(
			user,
			['scope changed'],
			sessionState(Date.now() + 1000, { sessionId: current.session.sessionId })
		);
		return 'authenticated';
	});
	assert.equal(await stable.runtime.onPolicyClose(() => true), 'reconnect');
	assert.equal(stable.refreshes, 1);
	const unavailable = await harness(async () => 'unavailable');
	assert.equal(await unavailable.runtime.onPolicyClose(() => true), 'retry');
	const denied = await harness(async (authStore) => {
		authStore.clearUser();
		return 'unauthenticated';
	});
	assert.equal(await denied.runtime.onPolicyClose(() => true), 'stop');
});

test('unknown or no longer owned identity fails closed before auth refresh', async () => {
	const unknown = await harness();
	unknown.authStore.clearUser();
	assert.equal(await unknown.runtime.onPolicyClose(() => true), 'stop');
	assert.equal(unknown.refreshes, 0);
	const retired = await harness();
	assert.equal(await retired.runtime.onPolicyClose(() => false), 'stop');
	assert.equal(retired.refreshes, 0);
});

for (const change of ['logout', 'user', 'session', 'term', 'draft', 'disposal']) {
	test(`store cannot reuse a delayed policy refresh after ${change}`, async () => {
		let finish;
		const fixture = await harness(
			() =>
				new Promise((resolve) => {
					finish = resolve;
				})
		);
		const recovery = fixture.runtime.onPolicyClose(() => true);
		for (let index = 0; index < 8 && !finish; index++) await Promise.resolve();
		assert.ok(finish);
		if (change === 'logout') fixture.authStore.clearUser();
		if (change === 'user')
			fixture.authStore.setUser({ ...user, id: 'user-b' }, ['*'], sessionState());
		if (change === 'session')
			fixture.authStore.setUser(
				user,
				['*'],
				sessionState(Date.now() + 1000, { sessionId: '01900000-0000-7000-8000-000000000002' })
			);
		if (change === 'term')
			fixture.store.connectTimetableSocket({ ...params, academicTermId: 'term-b' });
		if (change === 'draft')
			fixture.store.connectTimetableSocket({ ...params, timetableVersionId: 'draft-b' });
		if (change === 'disposal') fixture.store.disconnectTimetableSocket();
		finish('authenticated');
		assert.equal(await recovery, 'stop');
	});
}

test('only accepted draft or validated legacy state sync acknowledges the runtime baseline', async () => {
	const legacy = await harness();
	legacy.store.connectTimetableSocket({ ...params, timetableVersionId: undefined });
	for (const payload of [
		{ current_seq: '0', users: [] },
		{ current_seq: -1, users: [] },
		{ current_seq: 0, users: [{ user_id: 'known', name: 'missing-color' }] }
	])
		legacy.runtime.onMessage(JSON.stringify({ type: 'StateSync', payload }));
	assert.equal(legacy.confirmations, 0);
	legacy.runtime.onMessage(
		JSON.stringify({ type: 'StateSync', payload: { current_seq: 0, users: [] } })
	);
	assert.equal(legacy.confirmations, 1);
	for (const context of [null, { view_mode: 'homeroom', view_id: null }])
		legacy.runtime.onMessage(
			JSON.stringify({
				type: 'StateSync',
				payload: {
					current_seq: 0,
					users: [
						{ user_id: timetableIds.teacherA, name: 'legacy actor', color: '#123456', context }
					]
				}
			})
		);
	assert.equal(
		legacy.confirmations,
		3,
		'nullable legacy context matches the authoritative Rust Option wire'
	);
	const draft = await harness();
	const workspace = collaborationWorkspace();
	draft.store.connectTimetableSocket({ ...params, timetableVersionId: workspace.version.id });
	const snapshot = {
		epoch: timetableIds.blockA,
		seq: 0,
		type: 'DraftStateSync',
		payload: { connectionId: timetableIds.teacherA, workspace, users: [], reservations: [] }
	};
	draft.runtime.onMessage(
		JSON.stringify({
			...snapshot,
			payload: {
				...snapshot.payload,
				workspace: {
					...workspace,
					version: { ...workspace.version, id: timetableIds.publishedVersion }
				}
			}
		})
	);
	assert.equal(draft.confirmations, 0);
	draft.runtime.onMessage(JSON.stringify(snapshot));
	assert.equal(draft.confirmations, 1);
});
