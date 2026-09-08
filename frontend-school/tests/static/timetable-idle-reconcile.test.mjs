import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';
import { writable, get } from 'svelte/store';

test('timetable idle catches up after subscription even when restarted sequence is unchanged', async () => {
	let runtime;
	const dependencies = {
		'$lib/realtime/visibility-idle': { browserVisibilityDependencies: () => ({}) },
		'svelte/store': { writable },
		'$lib/api/client': {
			BACKEND_WS_URL: 'wss://school.example',
			getSchoolSubdomainHint: () => null
		},
		'$lib/realtime/auth-recovery': { realtimeAuthRecovery: async (refresh) => refresh() },
		'$lib/utils/timetable-socket-runtime': {
			createTimetableSocketRuntime: (config) => {
				runtime = config;
				return { connect() {}, disconnect() {} };
			}
		},
		'$lib/api/auth': { authAPI: { refreshCurrentUser: async () => 'authenticated' } }
	};
	const source = await readFile(
		new URL('../../src/lib/stores/timetable-socket.ts', import.meta.url),
		'utf8'
	);
	const compiled = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
	}).outputText;
	const exports = {};
	new Function('require', 'exports', compiled)((id) => {
		assert.ok(id in dependencies, id);
		return dependencies[id];
	}, exports);
	const sync = () =>
		runtime.onMessage(
			JSON.stringify({ type: 'StateSync', payload: { users: [], current_seq: 0 } })
		);
	sync();
	assert.equal(get(exports.refreshTrigger), 0);
	runtime.onPause();
	await runtime.onResume(() => true);
	assert.equal(get(exports.refreshTrigger), 0, 'must wait for authoritative subscription');
	sync();
	assert.equal(get(exports.refreshTrigger), 1, 'same sequence after restart still catches up');
	sync();
	assert.equal(get(exports.refreshTrigger), 1, 'catchup consumed once');
});
