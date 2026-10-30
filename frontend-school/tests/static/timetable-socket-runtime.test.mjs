import assert from 'node:assert/strict';
import test from 'node:test';
import { createTimetableSocketRuntime } from '../../src/lib/utils/timetable-socket-runtime.ts';

const termA = { academicTermId: 'term-a', currentUserId: 'user-1' };
const termB = { academicTermId: 'term-b', currentUserId: 'user-1' };

class FakeSocket {
	readyState = 0;
	onopen = null;
	onmessage = null;
	onclose = null;
	onerror = null;
	closed = false;
	sent = [];

	constructor(params) {
		this.params = params;
	}

	open() {
		this.readyState = 1;
		this.onopen?.({ type: 'open' });
	}

	message(data) {
		this.onmessage?.({ data });
	}

	closeFromServer(code = 1006, reason = '') {
		this.readyState = 3;
		this.onclose?.({ type: 'close', code, reason });
	}

	close() {
		this.closed = true;
		this.readyState = 3;
	}

	send(data) {
		this.sent.push(data);
	}
}

class FakeEnvironment {
	now = 0;
	nextTimerId = 1;
	timers = new Map();
	sockets = [];
	onlineListeners = new Set();
	online = true;
	hidden = false;
	visibilityListeners = new Set();

	setTimer = (callback, delay) => {
		const timerId = this.nextTimerId;
		this.nextTimerId += 1;
		this.timers.set(timerId, { callback, runAt: this.now + delay });
		return timerId;
	};

	clearTimer = (timerId) => {
		this.timers.delete(timerId);
	};

	createSocket = (params) => {
		const socket = new FakeSocket({ ...params });
		this.sockets.push(socket);
		return socket;
	};

	isOnline = () => this.online;

	addOnlineListener = (listener) => {
		this.onlineListeners.add(listener);
	};

	removeOnlineListener = (listener) => {
		this.onlineListeners.delete(listener);
	};

	nextDelay() {
		const runTimes = [...this.timers.values()].map(({ runAt }) => runAt - this.now);
		return runTimes.length === 0 ? null : Math.min(...runTimes);
	}

	advanceBy(duration) {
		const target = this.now + duration;

		while (true) {
			const nextTimer = [...this.timers.entries()]
				.filter(([, timer]) => timer.runAt <= target)
				.sort((left, right) => left[1].runAt - right[1].runAt)[0];
			if (!nextTimer) break;

			const [timerId, timer] = nextTimer;
			this.timers.delete(timerId);
			this.now = timer.runAt;
			timer.callback();
		}

		this.now = target;
	}

	goOnline() {
		this.online = true;
		for (const listener of [...this.onlineListeners]) listener();
	}
}

function createHarness(resume = async () => true, policyRecovery) {
	const environment = new FakeEnvironment();
	const notifications = {
		opens: 0,
		messages: [],
		closes: 0,
		closeEvents: [],
		errors: []
	};
	const runtime = createTimetableSocketRuntime({
		createSocket: environment.createSocket,
		setTimer: environment.setTimer,
		clearTimer: environment.clearTimer,
		isOnline: environment.isOnline,
		addOnlineListener: environment.addOnlineListener,
		removeOnlineListener: environment.removeOnlineListener,
		visibility: {
			isHidden: () => environment.hidden,
			addListener: (listener) => environment.visibilityListeners.add(listener),
			removeListener: (listener) => environment.visibilityListeners.delete(listener)
		},
		onResume: resume,
		onPolicyClose: policyRecovery,
		onPause: () => {},
		random: () => 0.5,
		onOpen: () => {
			notifications.opens += 1;
		},
		onMessage: (data) => {
			notifications.messages.push(data);
		},
		onClose: (event) => {
			notifications.closes += 1;
			notifications.closeEvents.push(event);
		},
		onError: (error) => {
			notifications.errors.push(error);
		}
	});

	return { environment, notifications, runtime };
}

test('repeated desired params within the debounce still replace a different active term', () => {
	const { environment, runtime } = createHarness();

	runtime.connect(termA);
	environment.advanceBy(50);
	const socketA = environment.sockets[0];
	socketA.open();

	runtime.connect(termB);
	environment.advanceBy(25);
	runtime.connect(termB);
	environment.advanceBy(50);

	assert.equal(environment.sockets.length, 2);
	assert.deepEqual(environment.sockets[1].params, termB);
	assert.equal(socketA.closed, true);
});

test('switching a draft within the same term replaces its socket and observer mode remains distinct', () => {
	const { environment, runtime } = createHarness();
	const first = { ...termA, timetableVersionId: 'draft-a' };
	const second = { ...termA, timetableVersionId: 'draft-b' };
	runtime.connect(first);
	environment.advanceBy(50);
	environment.sockets[0].open();
	runtime.connect(second);
	environment.advanceBy(50);
	assert.equal(environment.sockets[0].closed, true);
	assert.deepEqual(environment.sockets[1].params, second);
	environment.sockets[1].open();
	runtime.connect(termA);
	environment.advanceBy(50);
	assert.equal(environment.sockets[1].closed, true);
	assert.deepEqual(environment.sockets[2].params, termA);
});

test('replacement detaches every old handler and ignores already captured callbacks', () => {
	const { environment, notifications, runtime } = createHarness();

	runtime.connect(termA);
	environment.advanceBy(50);
	const socketA = environment.sockets[0];
	socketA.open();
	const staleCallbacks = {
		onopen: socketA.onopen,
		onmessage: socketA.onmessage,
		onclose: socketA.onclose,
		onerror: socketA.onerror
	};

	runtime.connect(termB);
	environment.advanceBy(50);
	staleCallbacks.onopen?.({ type: 'stale-open' });
	staleCallbacks.onmessage?.({ data: 'stale-message' });
	staleCallbacks.onclose?.({ type: 'stale-close' });
	staleCallbacks.onerror?.({ type: 'stale-error' });

	assert.deepEqual(
		[socketA.onopen, socketA.onmessage, socketA.onclose, socketA.onerror],
		[null, null, null, null]
	);
	assert.deepEqual(notifications, {
		opens: 1,
		messages: [],
		closes: 0,
		closeEvents: [],
		errors: []
	});
});

test('disconnect prevents stale socket callbacks from mutating or notifying', () => {
	const { environment, notifications, runtime } = createHarness();

	runtime.connect(termA);
	environment.advanceBy(50);
	const socket = environment.sockets[0];
	const staleCallbacks = {
		onopen: socket.onopen,
		onmessage: socket.onmessage,
		onclose: socket.onclose,
		onerror: socket.onerror
	};

	runtime.disconnect();
	staleCallbacks.onopen?.({ type: 'stale-open' });
	staleCallbacks.onmessage?.({ data: 'stale-message' });
	staleCallbacks.onclose?.({ type: 'stale-close' });
	staleCallbacks.onerror?.({ type: 'stale-error' });

	assert.equal(socket.closed, true);
	assert.deepEqual(
		[socket.onopen, socket.onmessage, socket.onclose, socket.onerror],
		[null, null, null, null]
	);
	assert.deepEqual(notifications, {
		opens: 0,
		messages: [],
		closes: 0,
		closeEvents: [],
		errors: []
	});
});

test('offline close registers one listener and reconnects once after returning online', () => {
	const { environment, runtime } = createHarness();

	runtime.connect(termA);
	environment.advanceBy(50);
	environment.online = false;
	environment.sockets[0].closeFromServer();

	assert.equal(environment.onlineListeners.size, 1);
	assert.equal(environment.timers.size, 0);
	environment.goOnline();
	assert.equal(environment.onlineListeners.size, 0);
	environment.advanceBy(50);
	assert.equal(environment.sockets.length, 2);

	environment.goOnline();
	environment.advanceBy(50);
	assert.equal(environment.sockets.length, 2);
});

test('failed sockets schedule increasing retries and reset backoff only after open', () => {
	const { environment, runtime } = createHarness();

	runtime.connect(termA);
	environment.advanceBy(50);
	environment.sockets[0].open();
	environment.sockets[0].closeFromServer();
	assert.equal(environment.nextDelay(), 1000);

	environment.advanceBy(1000);
	assert.equal(environment.nextDelay(), 50);
	environment.advanceBy(50);
	environment.sockets[1].closeFromServer();
	assert.equal(environment.nextDelay(), 2000);

	environment.advanceBy(2000);
	environment.advanceBy(50);
	environment.sockets[2].open();
	environment.sockets[2].closeFromServer();
	assert.equal(environment.nextDelay(), 1000);
});

test('explicit disconnect cancels pending reconnect timers and the online listener', () => {
	const pendingConnection = createHarness();
	pendingConnection.runtime.connect(termA);
	assert.equal(pendingConnection.environment.nextDelay(), 50);
	pendingConnection.runtime.disconnect();
	pendingConnection.environment.advanceBy(50);
	assert.equal(pendingConnection.environment.sockets.length, 0);

	const onlineRetry = createHarness();
	onlineRetry.runtime.connect(termA);
	onlineRetry.environment.advanceBy(50);
	onlineRetry.environment.sockets[0].closeFromServer();
	assert.equal(onlineRetry.environment.nextDelay(), 1000);

	onlineRetry.runtime.disconnect();
	assert.equal(onlineRetry.environment.timers.size, 0);
	onlineRetry.environment.advanceBy(5000);
	assert.equal(onlineRetry.environment.sockets.length, 1);

	const offlineRetry = createHarness();
	offlineRetry.runtime.connect(termA);
	offlineRetry.environment.advanceBy(50);
	offlineRetry.environment.online = false;
	offlineRetry.environment.sockets[0].closeFromServer();
	assert.equal(offlineRetry.environment.onlineListeners.size, 1);

	offlineRetry.runtime.disconnect();
	assert.equal(offlineRetry.environment.onlineListeners.size, 0);
	offlineRetry.environment.goOnline();
	offlineRetry.environment.advanceBy(5000);
	assert.equal(offlineRetry.environment.sockets.length, 1);
});

test('policy close suspends automatic reconnect until an explicit connect', () => {
	const { environment, notifications, runtime } = createHarness();

	runtime.connect(termA);
	environment.advanceBy(50);
	environment.sockets[0].open();
	environment.sockets[0].closeFromServer(1008, 'Authentication required');

	assert.equal(notifications.closes, 1);
	assert.equal(notifications.closeEvents[0].code, 1008);
	assert.equal(notifications.closeEvents[0].reason, 'Authentication required');
	assert.equal(environment.timers.size, 0);
	assert.equal(environment.onlineListeners.size, 0);
	environment.advanceBy(60_000);
	assert.equal(environment.sockets.length, 1);

	runtime.connect(termA);
	assert.equal(environment.nextDelay(), 50);
	environment.advanceBy(50);
	assert.equal(environment.sockets.length, 2);
});

test('realtime auth recovery maps authoritative refresh outcomes', async () => {
	const { realtimeAuthRecovery } = await import('../../src/lib/realtime/auth-recovery.ts');

	assert.equal(await realtimeAuthRecovery(async () => 'authenticated'), 'reconnect');
	assert.equal(await realtimeAuthRecovery(async () => 'unavailable'), 'retry');
	assert.equal(await realtimeAuthRecovery(async () => 'unauthenticated'), 'stop');
});

test('same-params refresh intent survives a later policy close exactly once', () => {
	const { environment, runtime } = createHarness();

	runtime.connect(termA);
	environment.advanceBy(50);
	const originalSocket = environment.sockets[0];
	originalSocket.open();

	// A permission-refresh event can request the same connection before the
	// backend policy close from the previous authorization reaches the browser.
	runtime.connect(termA);
	originalSocket.closeFromServer(1008);

	assert.equal(environment.nextDelay(), 50);
	environment.advanceBy(50);
	assert.equal(environment.sockets.length, 2);
	assert.deepEqual(environment.sockets[1].params, termA);

	environment.advanceBy(60_000);
	assert.equal(environment.sockets.length, 2);
});

function setHidden(environment, hidden) {
	environment.hidden = hidden;
	for (const listener of environment.visibilityListeners) listener();
}
test('hidden timetable pauses after grace and reconciles before reconnecting', async () => {
	let resumes = 0;
	const { environment, runtime } = createHarness(async () => {
		resumes++;
		return true;
	});
	runtime.connect(termA);
	environment.advanceBy(50);
	environment.sockets[0].open();
	setHidden(environment, true);
	environment.advanceBy(59_999);
	assert.equal(environment.sockets[0].closed, false);
	environment.advanceBy(1);
	assert.equal(environment.sockets[0].closed, true);
	environment.advanceBy(300_000);
	assert.equal(environment.sockets.length, 1);
	assert.equal(resumes, 0);
	runtime.connect(termB);
	environment.advanceBy(1000);
	assert.equal(environment.sockets.length, 1);
	setHidden(environment, false);
	await Promise.resolve();
	await Promise.resolve();
	environment.advanceBy(50);
	assert.equal(resumes, 1);
	assert.deepEqual(environment.sockets[1].params, termB);
	runtime.disconnect();
	assert.equal(environment.visibilityListeners.size, 0);
	assert.equal(environment.timers.size, 0);
});
test('logout during timetable resume cannot reconnect', async () => {
	let resolve;
	const { environment, runtime } = createHarness(() => new Promise((done) => (resolve = done)));
	runtime.connect(termA);
	environment.advanceBy(50);
	setHidden(environment, true);
	environment.advanceBy(60_000);
	setHidden(environment, false);
	runtime.disconnect();
	resolve(true);
	await Promise.resolve();
	await Promise.resolve();
	environment.advanceBy(60_000);
	assert.equal(environment.sockets.length, 1);
	assert.equal(environment.visibilityListeners.size, 0);
});

async function flushRecovery() {
	await Promise.resolve();
	await Promise.resolve();
}

test('authenticated policy recovery rejoins only after refresh on the still-current desired context', async () => {
	let finish;
	let current;
	const { environment, runtime } = createHarness(undefined, (isCurrent) => {
		current = isCurrent;
		return new Promise((resolve) => {
			finish = resolve;
		});
	});
	runtime.connect({ ...termA, timetableVersionId: 'draft-a' });
	environment.advanceBy(50);
	environment.sockets[0].open();
	environment.sockets[0].closeFromServer(1008);
	assert.equal(environment.sockets[0].onmessage, null);
	assert.equal(runtime.send('old-proof'), false);
	assert.equal(current(), true);
	assert.equal(environment.timers.size, 0);
	finish('reconnect');
	await flushRecovery();
	assert.equal(environment.nextDelay(), 50);
	environment.advanceBy(50);
	assert.equal(environment.sockets.length, 2);
	assert.deepEqual(environment.sockets[1].params, { ...termA, timetableVersionId: 'draft-a' });
});

test('unavailable policy auth uses the existing increasing backoff to retry auth before opening a socket', async () => {
	let attempts = 0;
	const { environment, runtime } = createHarness(undefined, async () =>
		++attempts < 3 ? 'retry' : 'reconnect'
	);
	runtime.connect(termA);
	environment.advanceBy(50);
	environment.sockets[0].open();
	environment.sockets[0].closeFromServer(1008);
	await flushRecovery();
	assert.equal(attempts, 1);
	assert.equal(environment.nextDelay(), 1000);
	environment.advanceBy(1000);
	await flushRecovery();
	assert.equal(attempts, 2);
	assert.equal(environment.sockets.length, 1);
	assert.equal(environment.nextDelay(), 2000);
	environment.advanceBy(2000);
	await flushRecovery();
	assert.equal(attempts, 3);
	assert.equal(environment.nextDelay(), 50);
	environment.advanceBy(50);
	assert.equal(environment.sockets.length, 2);
});

test('stopped policy recovery stays stopped and retains the legacy explicit-connect escape', async () => {
	const { environment, runtime } = createHarness(undefined, async () => 'stop');
	runtime.connect(termA);
	environment.advanceBy(50);
	environment.sockets[0].closeFromServer(1008);
	await flushRecovery();
	environment.advanceBy(60000);
	assert.equal(environment.sockets.length, 1);
	assert.equal(environment.timers.size, 0);
	assert.equal(environment.visibilityListeners.size, 0);
	runtime.connect(termA);
	environment.advanceBy(50);
	assert.equal(environment.sockets.length, 2);
});

for (const change of ['disconnect', 'term', 'draft', 'user', 'session replacement']) {
	test(`late policy auth cannot resurrect its old connection after ${change}`, async () => {
		let finish;
		let oldIsCurrent;
		const { environment, runtime } = createHarness(undefined, (isCurrent) => {
			oldIsCurrent = isCurrent;
			return new Promise((resolve) => {
				finish = resolve;
			});
		});
		runtime.connect({ ...termA, timetableVersionId: 'draft-a' });
		environment.advanceBy(50);
		environment.sockets[0].closeFromServer(1008);
		if (change === 'disconnect' || change === 'session replacement') runtime.disconnect();
		const next =
			change === 'term'
				? termB
				: change === 'draft'
					? { ...termA, timetableVersionId: 'draft-b' }
					: change === 'user'
						? { ...termA, currentUserId: 'user-2' }
						: { ...termA, timetableVersionId: 'draft-a' };
		if (change !== 'disconnect') runtime.connect(next);
		assert.equal(oldIsCurrent(), false);
		environment.advanceBy(50);
		finish('reconnect');
		await flushRecovery();
		environment.advanceBy(60000);
		assert.equal(environment.sockets.length, change === 'disconnect' ? 1 : 2);
		if (change !== 'disconnect') assert.deepEqual(environment.sockets[1].params, next);
	});
}

test('hidden pause invalidates in-flight policy recovery and resumes through the existing authenticated visibility path', async () => {
	let finish;
	let oldIsCurrent;
	let resumes = 0;
	const { environment, runtime } = createHarness(
		async () => {
			resumes++;
			return true;
		},
		(isCurrent) => {
			oldIsCurrent = isCurrent;
			return new Promise((resolve) => {
				finish = resolve;
			});
		}
	);
	runtime.connect(termA);
	environment.advanceBy(50);
	environment.sockets[0].closeFromServer(1008);
	setHidden(environment, true);
	environment.advanceBy(60000);
	assert.equal(oldIsCurrent(), false);
	finish('reconnect');
	await flushRecovery();
	assert.equal(environment.sockets.length, 1);
	assert.equal(environment.timers.size, 0);
	setHidden(environment, false);
	await flushRecovery();
	environment.advanceBy(50);
	assert.equal(resumes, 1);
	assert.equal(environment.sockets.length, 2);
});

test('offline policy close waits for the one existing online listener then revalidates auth', async () => {
	let attempts = 0;
	const { environment, runtime } = createHarness(undefined, async () => {
		attempts++;
		return 'reconnect';
	});
	runtime.connect(termA);
	environment.advanceBy(50);
	environment.online = false;
	environment.sockets[0].closeFromServer(1008);
	await flushRecovery();
	assert.equal(attempts, 0);
	assert.equal(environment.onlineListeners.size, 1);
	environment.goOnline();
	await flushRecovery();
	environment.advanceBy(50);
	assert.equal(attempts, 1);
	assert.equal(environment.onlineListeners.size, 0);
	assert.equal(environment.sockets.length, 2);
});

test('repeated policy denials retain capped backoff across TCP opens until the protocol confirms its baseline', async () => {
	let attempts = 0;
	const { environment, runtime } = createHarness(undefined, async () => {
		attempts++;
		return 'reconnect';
	});
	runtime.connect(termA);
	environment.advanceBy(50);
	environment.sockets[0].open();
	runtime.confirmStateSync();
	environment.sockets[0].closeFromServer(1008);
	await flushRecovery();
	environment.advanceBy(50);
	assert.equal(attempts, 1);
	for (const [index, delay] of [1000, 2000, 4000, 8000, 16000, 30000, 30000].entries()) {
		const socket = environment.sockets[index + 1];
		socket.open();
		socket.message('unvalidated traffic');
		socket.closeFromServer(1008);
		assert.equal(environment.nextDelay(), delay);
		environment.advanceBy(delay);
		await flushRecovery();
		environment.advanceBy(50);
	}
	const accepted = environment.sockets.at(-1);
	accepted.open();
	runtime.confirmStateSync();
	accepted.closeFromServer(1008);
	await flushRecovery();
	assert.equal(environment.nextDelay(), 50, 'a validated baseline starts a new recovery episode');
});

test('mixed transport1006 and policy1008 retries retain one policy episode until accepted state sync', async () => {
	let attempts = 0;
	const { environment, runtime } = createHarness(undefined, async () => {
		attempts++;
		return 'reconnect';
	});
	runtime.connect(termA);
	environment.advanceBy(50);
	environment.sockets[0].open();
	runtime.confirmStateSync();
	environment.sockets[0].closeFromServer(1008);
	await flushRecovery();
	environment.advanceBy(50);
	for (const [index, code] of [1006, 1006, 1008].entries()) {
		const socket = environment.sockets.at(-1);
		socket.open();
		socket.closeFromServer(code);
		assert.equal(
			environment.nextDelay(),
			[1000, 2000, 4000][index],
			`close ${code} must retain the pending-baseline attempt`
		);
		assert.equal(attempts, 1, 'transport retries and the policy delay do not start immediate auth');
		environment.advanceBy([1000, 2000, 4000][index]);
		await flushRecovery();
		environment.advanceBy(50);
	}
	assert.equal(attempts, 2);
	const accepted = environment.sockets.at(-1);
	accepted.open();
	runtime.confirmStateSync();
	accepted.closeFromServer(1006);
	assert.equal(environment.nextDelay(), 1000);
	environment.advanceBy(1000);
	environment.advanceBy(50);
	environment.sockets.at(-1).open();
	environment.sockets.at(-1).closeFromServer(1008);
	await flushRecovery();
	assert.equal(attempts, 3);
	assert.equal(environment.nextDelay(), 50);
});

test('visibility resume before a policy baseline preserves prior transport backoff and policy confirmation', async () => {
	let attempts = 0,
		resumes = 0;
	const { environment, runtime } = createHarness(
		async () => {
			resumes++;
			return true;
		},
		async () => {
			attempts++;
			return 'reconnect';
		}
	);
	runtime.connect(termA);
	environment.advanceBy(50);
	environment.sockets[0].open();
	runtime.confirmStateSync();
	environment.sockets[0].closeFromServer(1008);
	await flushRecovery();
	environment.advanceBy(50);
	environment.sockets.at(-1).open();
	environment.sockets.at(-1).closeFromServer(1006);
	assert.equal(environment.nextDelay(), 1000);
	environment.advanceBy(1000);
	environment.advanceBy(50);
	environment.sockets.at(-1).open();
	setHidden(environment, true);
	environment.advanceBy(60000);
	setHidden(environment, false);
	await flushRecovery();
	environment.advanceBy(50);
	assert.equal(resumes, 1);
	environment.sockets.at(-1).open();
	environment.sockets.at(-1).closeFromServer(1006);
	assert.equal(environment.nextDelay(), 2000);
	environment.advanceBy(2000);
	environment.advanceBy(50);
	environment.sockets.at(-1).open();
	environment.sockets.at(-1).closeFromServer(1008);
	assert.equal(environment.nextDelay(), 4000);
	assert.equal(attempts, 1);
	environment.advanceBy(4000);
	await flushRecovery();
	environment.advanceBy(50);
	environment.sockets.at(-1).open();
	runtime.confirmStateSync();
	environment.sockets.at(-1).closeFromServer(1006);
	assert.equal(environment.nextDelay(), 1000);
});

test('an explicit new owner context starts a clean policy episode and backoff before TCP open', async () => {
	let attempts = 0;
	const { environment, runtime } = createHarness(undefined, async () => {
		attempts++;
		return 'reconnect';
	});
	runtime.connect(termA);
	environment.advanceBy(50);
	environment.sockets[0].open();
	environment.sockets[0].closeFromServer(1008);
	await flushRecovery();
	environment.advanceBy(50);
	environment.sockets.at(-1).open();
	environment.sockets.at(-1).closeFromServer(1006);
	environment.advanceBy(1000);
	environment.advanceBy(50);
	environment.sockets.at(-1).open();
	environment.sockets.at(-1).closeFromServer(1006);
	const next = { ...termB, timetableVersionId: 'draft-b' };
	runtime.connect(next);
	environment.advanceBy(50);
	assert.deepEqual(environment.sockets.at(-1).params, next);
	environment.sockets.at(-1).closeFromServer(1006);
	assert.equal(
		environment.nextDelay(),
		1000,
		'a new owner intent discards the old context retry count'
	);
	environment.advanceBy(1000);
	environment.advanceBy(50);
	environment.sockets.at(-1).open();
	environment.sockets.at(-1).closeFromServer(1008);
	await flushRecovery();
	assert.equal(attempts, 2);
	assert.equal(environment.nextDelay(), 50);
});

test('online restoration preserves an unconfirmed policy episode after a transport disconnect', async () => {
	let attempts = 0;
	const { environment, runtime } = createHarness(undefined, async () => {
		attempts++;
		return 'reconnect';
	});
	runtime.connect(termA);
	environment.advanceBy(50);
	environment.sockets[0].open();
	runtime.confirmStateSync();
	environment.sockets[0].closeFromServer(1008);
	await flushRecovery();
	environment.advanceBy(50);
	environment.sockets.at(-1).open();
	environment.online = false;
	environment.sockets.at(-1).closeFromServer(1006);
	assert.equal(environment.onlineListeners.size, 1);
	environment.goOnline();
	environment.advanceBy(50);
	environment.sockets.at(-1).open();
	environment.sockets.at(-1).closeFromServer(1008);
	assert.equal(environment.nextDelay(), 1000);
	assert.equal(attempts, 1);
	environment.advanceBy(1000);
	await flushRecovery();
	environment.advanceBy(50);
	assert.equal(attempts, 2);
	assert.equal(environment.onlineListeners.size, 0);
});
