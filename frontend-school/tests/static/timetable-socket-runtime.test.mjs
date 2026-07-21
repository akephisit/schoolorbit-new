import assert from 'node:assert/strict';
import test from 'node:test';
import { createTimetableSocketRuntime } from '../../src/lib/utils/timetable-socket-runtime.ts';

const semesterA = { semester_id: 'semester-a', current_user_id: 'user-1' };
const semesterB = { semester_id: 'semester-b', current_user_id: 'user-1' };

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

	closeFromServer(code = 1006) {
		this.readyState = 3;
		this.onclose?.({ type: 'close', code });
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

function createHarness() {
	const environment = new FakeEnvironment();
	const notifications = {
		opens: 0,
		messages: [],
		closes: 0,
		errors: []
	};
	const runtime = createTimetableSocketRuntime({
		createSocket: environment.createSocket,
		setTimer: environment.setTimer,
		clearTimer: environment.clearTimer,
		isOnline: environment.isOnline,
		addOnlineListener: environment.addOnlineListener,
		removeOnlineListener: environment.removeOnlineListener,
		random: () => 0.5,
		onOpen: () => {
			notifications.opens += 1;
		},
		onMessage: (data) => {
			notifications.messages.push(data);
		},
		onClose: () => {
			notifications.closes += 1;
		},
		onError: (error) => {
			notifications.errors.push(error);
		}
	});

	return { environment, notifications, runtime };
}

test('repeated desired params within the debounce still replace a different active semester', () => {
	const { environment, runtime } = createHarness();

	runtime.connect(semesterA);
	environment.advanceBy(50);
	const socketA = environment.sockets[0];
	socketA.open();

	runtime.connect(semesterB);
	environment.advanceBy(25);
	runtime.connect(semesterB);
	environment.advanceBy(50);

	assert.equal(environment.sockets.length, 2);
	assert.deepEqual(environment.sockets[1].params, semesterB);
	assert.equal(socketA.closed, true);
});

test('replacement detaches every old handler and ignores already captured callbacks', () => {
	const { environment, notifications, runtime } = createHarness();

	runtime.connect(semesterA);
	environment.advanceBy(50);
	const socketA = environment.sockets[0];
	socketA.open();
	const staleCallbacks = {
		onopen: socketA.onopen,
		onmessage: socketA.onmessage,
		onclose: socketA.onclose,
		onerror: socketA.onerror
	};

	runtime.connect(semesterB);
	environment.advanceBy(50);
	staleCallbacks.onopen?.({ type: 'stale-open' });
	staleCallbacks.onmessage?.({ data: 'stale-message' });
	staleCallbacks.onclose?.({ type: 'stale-close' });
	staleCallbacks.onerror?.({ type: 'stale-error' });

	assert.deepEqual(
		[socketA.onopen, socketA.onmessage, socketA.onclose, socketA.onerror],
		[null, null, null, null]
	);
	assert.deepEqual(notifications, { opens: 1, messages: [], closes: 0, errors: [] });
});

test('disconnect prevents stale socket callbacks from mutating or notifying', () => {
	const { environment, notifications, runtime } = createHarness();

	runtime.connect(semesterA);
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
	assert.deepEqual(notifications, { opens: 0, messages: [], closes: 0, errors: [] });
});

test('offline close registers one listener and reconnects once after returning online', () => {
	const { environment, runtime } = createHarness();

	runtime.connect(semesterA);
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

	runtime.connect(semesterA);
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
	pendingConnection.runtime.connect(semesterA);
	assert.equal(pendingConnection.environment.nextDelay(), 50);
	pendingConnection.runtime.disconnect();
	pendingConnection.environment.advanceBy(50);
	assert.equal(pendingConnection.environment.sockets.length, 0);

	const onlineRetry = createHarness();
	onlineRetry.runtime.connect(semesterA);
	onlineRetry.environment.advanceBy(50);
	onlineRetry.environment.sockets[0].closeFromServer();
	assert.equal(onlineRetry.environment.nextDelay(), 1000);

	onlineRetry.runtime.disconnect();
	assert.equal(onlineRetry.environment.timers.size, 0);
	onlineRetry.environment.advanceBy(5000);
	assert.equal(onlineRetry.environment.sockets.length, 1);

	const offlineRetry = createHarness();
	offlineRetry.runtime.connect(semesterA);
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

	runtime.connect(semesterA);
	environment.advanceBy(50);
	environment.sockets[0].open();
	environment.sockets[0].closeFromServer(1008);

	assert.equal(notifications.closes, 1);
	assert.equal(environment.timers.size, 0);
	assert.equal(environment.onlineListeners.size, 0);
	environment.advanceBy(60_000);
	assert.equal(environment.sockets.length, 1);

	runtime.connect(semesterA);
	assert.equal(environment.nextDelay(), 50);
	environment.advanceBy(50);
	assert.equal(environment.sockets.length, 2);
});

test('same-params refresh intent survives a later policy close exactly once', () => {
	const { environment, runtime } = createHarness();

	runtime.connect(semesterA);
	environment.advanceBy(50);
	const originalSocket = environment.sockets[0];
	originalSocket.open();

	// A permission-refresh event can request the same connection before the
	// backend policy close from the previous authorization reaches the browser.
	runtime.connect(semesterA);
	originalSocket.closeFromServer(1008);

	assert.equal(environment.nextDelay(), 50);
	environment.advanceBy(50);
	assert.equal(environment.sockets.length, 2);
	assert.deepEqual(environment.sockets[1].params, semesterA);

	environment.advanceBy(60_000);
	assert.equal(environment.sockets.length, 2);
});
