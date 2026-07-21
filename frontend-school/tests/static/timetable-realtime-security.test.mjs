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
const connectionContract = store.slice(
	store.indexOf('type TimetableSocketParams'),
	store.indexOf('export function disconnectTimetableSocket')
);
const disconnectContract = store.slice(
	store.indexOf('export function disconnectTimetableSocket'),
	store.indexOf('export function sendTimetableEvent')
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
	assert.match(connectionContract, /new WebSocket\(url\)/);
});

test('reconnect is exponential, capped, jittered, offline aware, and explicitly cancellable', async () => {
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
	assert.match(store, /window\.addEventListener\('online'/);
	assert.match(store, /window\.removeEventListener\('online'/);
	assert.match(store, /if\s*\(!waitingForOnline\)/);
	assert.match(store, /reconnectAttempt\s*=\s*0/);
	assert.match(disconnectContract, /clearOnlineListener\(\)/);
	assert.match(disconnectContract, /isConnected\.set\(false\)/);
	assert.match(disconnectContract, /activeUsers\.set\(\[\]\)/);
	assert.match(disconnectContract, /currentUserId\s*=\s*null/);
	assert.match(disconnectContract, /lastParams\s*=\s*null/);
});

test('page passes server query and local-only current user identity', () => {
	assert.match(page, /connectTimetableSocket\(\{\s*semester_id:[\s\S]*current_user_id:/);
	assert.doesNotMatch(page, /connectTimetableSocket\(\{[\s\S]{0,180}name:/);
});
