import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const store = readFileSync(
	new URL('../../src/lib/stores/timetable-socket.ts', import.meta.url),
	'utf8'
);
const page = readFileSync(
	new URL('../../src/routes/(app)/staff/academic/timetable/+page.svelte', import.meta.url),
	'utf8'
);
const runtime = readFileSync(
	new URL('../../src/lib/utils/timetable-socket-runtime.ts', import.meta.url),
	'utf8'
);
const connectionContract = store.slice(
	store.indexOf('const timetableSocketRuntime'),
	store.indexOf('export function sendUserActivity')
);
const pageConnectionEffect = page.slice(
	page.indexOf('// WebSocket Connection'),
	page.indexOf('onDestroy(() =>')
);

test('timetable socket URL contains only semester identity', () => {
	assert.match(
		connectionContract,
		/new URLSearchParams\(\{\s*semester_id:\s*String\(params\.semester_id\)\s*\}\)/s
	);
	assert.doesNotMatch(connectionContract, /school_key\s*:/);
	assert.doesNotMatch(connectionContract, /name:\s*string/);
	assert.doesNotMatch(connectionContract, /user_id:\s*String\(params\.user_id\)/);
	assert.doesNotMatch(connectionContract, /current_user_id:\s*String/);
	assert.doesNotMatch(connectionContract, /\btoken\b/i);
	assert.doesNotMatch(connectionContract, /(?:localStorage|sessionStorage)/);
	assert.doesNotMatch(connectionContract, /console\.(?:log|info|debug|warn|error)\([^)]*\burl\b/);
	assert.match(connectionContract, /return new WebSocket\(url\)/);
});

test('reconnect delay remains exponential, capped, and jittered', async () => {
	const { reconnectDelayMs } = await import('../../src/lib/utils/timetable-reconnect.ts');

	assert.deepEqual(
		[0, 1, 2, 3, 4].map((attempt) => reconnectDelayMs(attempt, () => 0.5)),
		[1000, 2000, 4000, 8000, 16000]
	);
	assert.equal(
		reconnectDelayMs(8, () => 0.5),
		30000
	);
	assert.equal(
		reconnectDelayMs(0, () => 0),
		800
	);
	assert.equal(
		reconnectDelayMs(0, () => 1),
		1200
	);
});

test('store delegates socket ownership, timers, and network listeners to the runtime', () => {
	assert.match(store, /createTimetableSocketRuntime\(\{/);
	assert.match(connectionContract, /timetableSocketRuntime\.connect\(params\)/);
	assert.match(connectionContract, /timetableSocketRuntime\.disconnect\(\)/);
	assert.match(connectionContract, /timetableSocketRuntime\.send\(JSON\.stringify\(event\)\)/);
	assert.match(runtime, /socketGeneration/);
	assert.match(runtime, /detachSocketHandlers/);
});

test('page passes server query and local-only current user identity', () => {
	assert.match(
		pageConnectionEffect,
		/connectTimetableSocket\(\{\s*semester_id:[\s\S]*current_user_id:/
	);
	assert.doesNotMatch(pageConnectionEffect, /connectTimetableSocket\(\{[\s\S]{0,180}name:/);
	assert.match(
		pageConnectionEffect,
		/if \(canReadTimetable && selectedSemesterId && \$authStore\.user\) \{[\s\S]*\} else \{\s*disconnectTimetableSocket\(\);\s*\}/
	);
});
