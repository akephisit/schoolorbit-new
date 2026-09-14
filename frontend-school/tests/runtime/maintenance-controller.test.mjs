import assert from 'node:assert/strict';
import test from 'node:test';

import {
	createDeploymentMonitor,
	isMaintenanceResponse
} from '../../src/lib/deployment/maintenance-controller.ts';

const maintenance = {
	status: 'maintenance',
	releaseId: '0123456789abcdef0123456789abcdef01234567',
	retryAfterSeconds: 10
};
const ready = { ...maintenance, status: 'ready' };

test('only the typed 503 maintenance envelope confirms a deployment interruption', () => {
	assert.equal(isMaintenanceResponse(503, { success: false, error: 'maintenance' }), true);
	assert.equal(
		isMaintenanceResponse(503, { success: false, error: 'database_unavailable' }),
		false
	);
	assert.equal(isMaintenanceResponse(500, { success: false, error: 'maintenance' }), false);
	assert.equal(isMaintenanceResponse(503, 'maintenance'), false);
});

function fixture(statuses) {
	const scheduled = [];
	const storage = new Map();
	const states = [];
	let visible;
	let reloads = 0;
	const monitor = createDeploymentMonitor({
		readStatus: async () => {
			const next = statuses.shift();
			if (next instanceof Error) throw next;
			if (!next) throw new Error('unexpected status read');
			return next;
		},
		schedule: (callback, delay) => {
			scheduled.push({ callback, delay });
			return scheduled.length;
		},
		cancelSchedule: () => {},
		reload: () => {
			reloads += 1;
		},
		storage: {
			getItem: (key) => storage.get(key) ?? null,
			setItem: (key, value) => storage.set(key, value)
		},
		onVisible: (callback) => {
			visible = callback;
			return () => {
				visible = undefined;
			};
		}
	});
	monitor.subscribe((state) => states.push(state));
	return {
		monitor,
		scheduled,
		states,
		storage,
		visible: () => visible,
		reloads: () => reloads
	};
}

test('confirmed maintenance replaces the app and schedules the next status check', async () => {
	const context = fixture([maintenance]);

	await context.monitor.start();

	assert.deepEqual(context.states.at(-1), {
		status: 'maintenance',
		releaseId: maintenance.releaseId
	});
	assert.equal(context.scheduled.length, 1);
	assert.equal(context.scheduled[0].delay, 10_000);
});

test('an initial status network failure does not falsely enter maintenance', async () => {
	const context = fixture([new Error('offline')]);

	await context.monitor.start();

	assert.deepEqual(context.states, [{ status: 'ready', releaseId: null }]);
	assert.equal(context.scheduled.length, 0);
});

test('maintenance remains visible when a poll fails and retries after ten seconds', async () => {
	const context = fixture([maintenance, new Error('temporary')]);
	await context.monitor.start();

	await context.scheduled.shift().callback();

	assert.equal(context.states.at(-1).status, 'maintenance');
	assert.equal(context.scheduled.length, 1);
	assert.equal(context.scheduled[0].delay, 10_000);
});

test('a ready release reloads once after maintenance even when confirmed again', async () => {
	const context = fixture([maintenance, ready, ready]);
	await context.monitor.start();

	await context.scheduled.shift().callback();
	await context.monitor.confirmMaintenance();

	assert.equal(context.reloads(), 1);
	assert.equal(context.storage.get(`schoolorbit:deployment-reloaded:${ready.releaseId}`), '1');
});

test('maintenance rearms reload when the same commit is deployed again', async () => {
	const context = fixture([maintenance, ready]);
	const guardKey = `schoolorbit:deployment-reloaded:${ready.releaseId}`;
	context.storage.set(guardKey, '1');

	await context.monitor.start();
	await context.scheduled.shift().callback();

	assert.equal(context.reloads(), 1);
	assert.equal(context.storage.get(guardKey), '1');
});

test('returning to a visible tab checks a confirmed maintenance release immediately', async () => {
	const context = fixture([maintenance, ready]);
	await context.monitor.start();

	await context.visible()();

	assert.equal(context.reloads(), 1);
	context.monitor.stop();
	assert.equal(context.visible(), undefined);
});

test('a tab hidden for the whole release reloads when the ready release changes', async () => {
	const previousRelease = {
		...ready,
		releaseId: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
	};
	const context = fixture([previousRelease, ready]);

	await context.monitor.start();
	await context.visible()();

	assert.equal(context.reloads(), 1);
});

test('stopping during a pending status request prevents late state and timer changes', async () => {
	let finishRead;
	const scheduled = [];
	const states = [];
	const monitor = createDeploymentMonitor({
		readStatus: () => new Promise((resolve) => (finishRead = resolve)),
		schedule: (callback, delay) => {
			scheduled.push({ callback, delay });
			return scheduled.length;
		},
		cancelSchedule: () => {},
		reload: () => assert.fail('a stopped monitor must not reload'),
		storage: { getItem: () => null, setItem: () => {} },
		onVisible: () => () => {}
	});
	monitor.subscribe((state) => states.push(state));

	const start = monitor.start();
	monitor.stop();
	finishRead(maintenance);
	await start;

	assert.deepEqual(states, [{ status: 'ready', releaseId: null }]);
	assert.equal(scheduled.length, 0);
});
