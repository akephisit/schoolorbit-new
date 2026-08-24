import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const store = readFileSync(
	new URL('../../src/lib/stores/timetable-socket.ts', import.meta.url),
	'utf8'
);
const runtime = readFileSync(
	new URL('../../src/lib/utils/timetable-socket-runtime.ts', import.meta.url),
	'utf8'
);
const client = readFileSync(new URL('../../src/lib/api/client.ts', import.meta.url), 'utf8');
const connectionContract = store.slice(store.indexOf('const timetableSocketRuntime'));
const onCloseContract = connectionContract.slice(
	connectionContract.indexOf('onClose:'),
	connectionContract.indexOf('onError:')
);

test('timetable socket URL contains canonical term identity and one sanitized tenant hint', () => {
	assert.match(
		connectionContract,
		/new URL\(['"]\/ws\/timetable['"],\s*BACKEND_WS_URL\)[\s\S]*searchParams\.set\(\s*['"]academicTermId['"],\s*String\(params\.academicTermId\)\s*\)/
	);
	assert.match(connectionContract, /getSchoolSubdomainHint\(\)/);
	assert.match(
		connectionContract,
		/searchParams\.set\(['"]school_subdomain['"],\s*schoolSubdomain\)/
	);
	assert.equal([...connectionContract.matchAll(/['"]school_subdomain['"]/g)].length, 1);
	assert.doesNotMatch(connectionContract, /school_key\s*:/);
	assert.doesNotMatch(connectionContract, /name:\s*string/);
	assert.doesNotMatch(connectionContract, /user_id:\s*String\(params\.user_id\)/);
	assert.doesNotMatch(connectionContract, /current_user_id:\s*String/);
	assert.doesNotMatch(connectionContract, /\btoken\b/i);
	assert.doesNotMatch(connectionContract, /(?:localStorage|sessionStorage)/);
	assert.doesNotMatch(connectionContract, /console\.(?:log|info|debug|warn|error)\([^)]*\burl\b/);
	assert.match(connectionContract, /return new WebSocket\(url\)/);
});

test('client tenant hint rejects values that cannot be a school subdomain', async () => {
	assert.match(client, /export function getSchoolSubdomainHint\(\):\s*string \| null/);
	assert.match(
		client,
		/getSchoolSubdomainHint\(\)[\s\S]*normalizeSchoolSubdomain\(env\.PUBLIC_SCHOOL_SUBDOMAIN\)/
	);

	const { normalizeSchoolSubdomain } = await import('../../src/lib/api/school-subdomain.ts');
	assert.equal(normalizeSchoolSubdomain('School-A'), 'school-a');
	for (const value of [
		'',
		'www',
		' tenant',
		'tenant ',
		'-tenant',
		'tenant-',
		'a'.repeat(64),
		'tenant.example.com',
		'tenant/name',
		'tenant?x=1',
		'a_b',
		'โรงเรียน'
	]) {
		assert.equal(normalizeSchoolSubdomain(value), null, value);
	}
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

test('canonical store delegates socket ownership, timers, and network listeners to the runtime', () => {
	assert.match(store, /createTimetableSocketRuntime\(\{/);
	assert.match(connectionContract, /timetableSocketRuntime\.connect\(params\)/);
	assert.match(connectionContract, /timetableSocketRuntime\.disconnect\(\)/);
	assert.match(store, /timetableSocketRuntime\.send\(JSON\.stringify\(event\)\)/);
	assert.match(runtime, /socketGeneration/);
	assert.match(runtime, /detachSocketHandlers/);
});

test('policy close refreshes auth without clearing or blindly reconnecting', () => {
	assert.match(onCloseContract, /onClose:\s*\(event\)/);
	assert.match(onCloseContract, /clearRealtimeState\(\)/);
	assert.match(onCloseContract, /event\.code\s*!==\s*1008/);
	assert.match(onCloseContract, /recoverTimetableAuth\(\)/);
	assert.match(
		store,
		/realtimeAuthRecovery\(\(\)\s*=>\s*authAPI\.refreshCurrentUser\(\{\s*silent:\s*true\s*\}\)\)/
	);
	assert.doesNotMatch(onCloseContract, /clearUser|clearSessionSecurity|\.connect\(/);
});

test('socket runtime keeps term and local user identity explicit without legacy query keys', () => {
	assert.match(runtime, /academicTermId:\s*string/);
	assert.match(runtime, /currentUserId:\s*string/);
	assert.match(connectionContract, /currentUserId = params\.currentUserId/);
	assert.doesNotMatch(store, /semester_id|current_user_id|timetable\/replay/);
});

test('timetable realtime uses canonical reload signals without legacy optimistic wire DTOs', () => {
	for (const signal of ['AcademicCoreChanged', 'LearningDeliveryChanged', 'TimetableChanged']) {
		assert.match(store, new RegExp(`['"]${signal}['"]`));
	}
	assert.match(store, /refreshTrigger\.update/);
	assert.match(store, /export function sendCursorMove/);
	assert.doesNotMatch(
		store,
		/classroom_course|activity_slot|CourseTeamChanged|EntryCreated|DropIntent|EntryIntent|sendDrop/
	);
});
