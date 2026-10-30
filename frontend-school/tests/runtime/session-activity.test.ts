import assert from 'node:assert/strict';
import test from 'node:test';
import {
	createSessionActivityController,
	ACTIVITY_WINDOW_MS,
	type ActivityResult,
	type SessionState,
	type SessionWarning
} from '../../src/lib/auth/session-activity.ts';
import {
	createSessionActivityCoordinator,
	sessionActivityKeys,
	readSharedSession
} from '../../src/lib/auth/session-activity-coordination.ts';

const start = Date.parse('2026-10-29T00:00:00Z');
const sessionId = '01900000-0000-7000-8000-000000000001';
const date = (offset: number) => new Date(start + offset).toISOString();
const state = (now = 0, overrides: Partial<SessionState> = {}): SessionState => ({
	sessionId,
	serverNow: date(now),
	idleExpiresAt: date(now + 7_200_000),
	absoluteExpiresAt: date(43_200_000),
	nextRenewalAt: date(now + ACTIVITY_WINDOW_MS),
	...overrides
});

function harness(hold = false, initial = state()) {
	let now = 0;
	let wall = start;
	let rechecks = 0;
	let validation: (() => Promise<boolean>) | undefined;
	const timers = new Map<ReturnType<typeof setTimeout>, { due: number; callback(): void }>();
	const requests: {
		session: SessionState;
		signal: AbortSignal;
		resolve(value: ActivityResult): void;
	}[] = [];
	const warnings: (SessionWarning | null)[] = [];
	const accepted: SessionState[] = [];
	const controller = createSessionActivityController({
		now: () => now,
		wallNow: () => wall,
		async resyncClock() {
			rechecks++;
			return validation ? validation() : true;
		},
		setTimer(callback, delay) {
			const timer = setTimeout(() => {}, 1_000_000_000);
			clearTimeout(timer);
			timers.set(timer, { due: now + delay, callback });
			return timer;
		},
		clearTimer(timer) {
			timers.delete(timer);
		},
		renew(session, signal) {
			return new Promise((resolve) => {
				requests.push({ session, signal, resolve });
				if (!hold) resolve({ kind: 'renewed', session: state(now) });
			});
		},
		accept(session) {
			accepted.push(session);
			controller.setSession(session);
		},
		warning(value) {
			warnings.push(value);
		}
	});
	controller.setSession(initial);
	controller.setEnvironment(true, true);
	return {
		controller,
		requests,
		accepted,
		warnings,
		get now() {
			return now;
		},
		get rechecks() {
			return rechecks;
		},
		validateWith(next: () => Promise<boolean>) {
			validation = next;
		},
		setWall(value: number) {
			wall = value;
		},
		async advance(delta: number) {
			const target = now + delta;
			let iterations = 0;
			while (true) {
				const next = [...timers.entries()].sort((a, b) => a[1].due - b[1].due)[0];
				if (!next || next[1].due > target) break;
				assert.ok(iterations++ < 100, 'timer must not busy loop');
				wall += next[1].due - now;
				now = next[1].due;
				timers.delete(next[0]);
				next[1].callback();
				for (let i = 0; i < 8; i++) await Promise.resolve();
			}
			wall += target - now;
			now = target;
			for (let i = 0; i < 8; i++) await Promise.resolve();
		},
		jump(delta: number) {
			now += delta;
			wall += delta;
			controller.wake();
		}
	};
}

test('mount, passive reading and visible focus alone never renew; warning is central', async () => {
	const h = harness();
	await h.advance(116 * 60_000);
	assert.equal(h.requests.length, 0);
	assert.equal(h.warnings.at(-1)?.remainingMinutes, 4);
	h.controller.setEnvironment(true, true);
	await h.advance(60_000);
	assert.equal(h.requests.length, 0);
	h.controller.dispose();
});

test('event storms stay local and one fresh input renews at the server cadence', async () => {
	const h = harness();
	for (let i = 0; i < 10_000; i++) h.controller.trustedActivity();
	assert.equal(h.requests.length, 0);
	await h.advance(ACTIVITY_WINDOW_MS);
	assert.equal(h.requests.length, 1);
	await h.advance(ACTIVITY_WINDOW_MS * 2);
	assert.equal(h.requests.length, 1, 'old input cannot keep a page alive');
	h.controller.trustedActivity();
	await h.advance(1);
	assert.equal(h.requests.length, 2);
	h.controller.dispose();
});

test('new input arriving during an outstanding renewal survives its completion', async () => {
	const h = harness(true);
	h.controller.trustedActivity();
	await h.advance(ACTIVITY_WINDOW_MS);
	await h.advance(10);
	h.controller.trustedActivity();
	h.requests[0].resolve({ kind: 'renewed', session: state(h.now) });
	await h.advance(0);
	await h.advance(ACTIVITY_WINDOW_MS);
	assert.equal(h.requests.length, 2);
	h.controller.dispose();
});

test('hidden/offline input and returning visibility do not renew; fresh input can', async () => {
	const h = harness();
	h.controller.trustedActivity();
	h.controller.setEnvironment(false, true);
	await h.advance(ACTIVITY_WINDOW_MS);
	h.controller.setEnvironment(true, false);
	h.controller.trustedActivity();
	await h.advance(ACTIVITY_WINDOW_MS);
	h.controller.setEnvironment(true, true);
	await h.advance(1);
	assert.equal(h.requests.length, 0);
	h.controller.trustedActivity();
	await h.advance(1);
	assert.equal(h.requests.length, 1);
	h.controller.dispose();
});

test('sleep/wake drops old input and sends no catch-up burst', async () => {
	const h = harness();
	h.controller.trustedActivity();
	h.jump(ACTIVITY_WINDOW_MS * 10);
	await h.advance(1);
	assert.equal(h.requests.length, 0);
	h.controller.trustedActivity();
	await h.advance(1);
	assert.equal(h.requests.length, 1);
	h.controller.dispose();
});

test('transient failure honors backoff and Retry-After without discarding authentication', async () => {
	const h = harness(true);
	h.controller.trustedActivity();
	await h.advance(ACTIVITY_WINDOW_MS - 1);
	h.controller.trustedActivity();
	await h.advance(1);
	h.requests[0].resolve({ kind: 'retry', retryAfterMs: 20_000 });
	await h.advance(0);
	await h.advance(19_999);
	assert.equal(h.requests.length, 1);
	await h.advance(2);
	assert.equal(h.requests.length, 2);
	assert.equal(h.accepted.length, 0);
	h.controller.dispose();
});

test('same-user session replacement and disposal abort and fence late responses', async () => {
	for (const action of ['replace', 'dispose']) {
		const h = harness(true);
		h.controller.trustedActivity();
		await h.advance(ACTIVITY_WINDOW_MS);
		if (action === 'replace')
			h.controller.setSession(state(h.now, { sessionId: '01900000-0000-7000-8000-000000000002' }));
		else h.controller.dispose();
		assert.equal(h.requests[0].signal.aborted, true);
		h.requests[0].resolve({ kind: 'renewed', session: state(h.now) });
		await h.advance(1);
		assert.equal(h.accepted.length, 0);
		h.controller.dispose();
	}
});

test('local clock forward/rewind triggers validation without advancing monotonic authority', async () => {
	const h = harness();
	h.setWall(Date.parse('2099-01-01T00:00:00Z'));
	h.controller.wake();
	await h.advance(0);
	assert.equal(h.rechecks, 1);
	h.setWall(start - 3_600_000);
	h.controller.wake();
	await h.advance(0);
	assert.equal(h.rechecks, 2);
	h.controller.trustedActivity();
	await h.advance(ACTIVITY_WINDOW_MS - 1);
	assert.equal(h.requests.length, 0);
	await h.advance(1);
	assert.equal(h.requests.length, 1);
	h.controller.dispose();
});

test('OS suspend with a paused monotonic clock discards old input and validates expiry once', async () => {
	const h = harness();
	h.controller.trustedActivity();
	h.setWall(start + 3 * 3_600_000);
	h.validateWith(async () => {
		h.controller.setSession(null);
		return false;
	});
	h.controller.wake();
	h.controller.wake();
	await h.advance(0);
	assert.equal(h.rechecks, 1);
	assert.equal(h.requests.length, 0, 'hours-old input must never POST activity');
	assert.equal(h.warnings.at(-1), null, 'the central auth owner handled the actual expiry');
	h.controller.dispose();
});

test('sleep revalidation corrects deadlines and fresh input remains required', async () => {
	const h = harness();
	h.controller.trustedActivity();
	h.setWall(start + 3 * 3_600_000);
	h.validateWith(async () => {
		h.controller.setSession(state(3 * 3_600_000, { idleExpiresAt: date(3 * 3_600_000 + 180_000) }));
		return true;
	});
	h.controller.wake();
	await h.advance(0);
	assert.equal(h.rechecks, 1);
	assert.equal(h.requests.length, 0);
	assert.equal(h.warnings.at(-1)?.remainingMinutes, 3);
	h.controller.dispose();
});

test('a queued timer detects suspend without wake or new input and never replays old activity', async () => {
	const h = harness();
	await h.advance(ACTIVITY_WINDOW_MS - 1_000);
	h.controller.trustedActivity();
	h.setWall(start + h.now + 3_600_000);
	h.validateWith(async () => {
		h.controller.setSession(
			state(ACTIVITY_WINDOW_MS + 3_600_000, {
				idleExpiresAt: date(7_200_000),
				nextRenewalAt: date(ACTIVITY_WINDOW_MS)
			})
		);
		return true;
	});
	// Only the previously queued one-second timer resumes; no pageshow/focus/input occurs.
	await h.advance(1_000);
	assert.equal(h.rechecks, 1);
	assert.equal(h.requests.length, 0, 'the monotonic clock cannot make pre-suspend input fresh');
	await h.advance(ACTIVITY_WINDOW_MS);
	assert.equal(h.rechecks, 1);
	assert.equal(h.requests.length, 0);
	h.controller.dispose();
});

test('absolute expiry warning explains the unextendable limit', async () => {
	const h = harness(false, state(0, { absoluteExpiresAt: date(180_000) }));
	assert.equal(h.warnings.at(-1)?.kind, 'absolute');
	assert.equal(h.warnings.at(-1)?.remainingMinutes, 3);
	h.controller.dispose();
});

test('delayed shared metadata cannot move the estimated server clock backwards', async () => {
	const h = harness();
	await h.advance(240_000);
	h.controller.setSession(state(60_000, { nextRenewalAt: date(360_000) }));
	h.controller.trustedActivity();
	await h.advance(120_000);
	assert.equal(h.requests.length, 1);
	h.controller.dispose();
});

function sharedHarness() {
	const map = new Map<string, string>();
	const storage = {
		getItem: (key: string) => map.get(key) ?? null,
		setItem(key: string, value: string) {
			map.set(key, value);
		},
		removeItem(key: string) {
			map.delete(key);
		}
	};
	return { map, storage };
}

test('fallback lease coalesces simultaneous tabs and re-reads successful metadata', async () => {
	const shared = sharedHarness();
	const deps = {
		storage: shared.storage,
		now: () => start,
		random: () => 0.4,
		wait: async () => {
			await Promise.resolve();
		}
	};
	const a = createSessionActivityCoordinator(deps),
		b = createSessionActivityCoordinator(deps);
	let calls = 0;
	const renew = async (): Promise<ActivityResult> => {
		calls++;
		return { kind: 'renewed', session: state(ACTIVITY_WINDOW_MS) };
	};
	const signal = new AbortController().signal;
	await Promise.all([
		a.run('https://school.invalid', state(), signal, renew),
		b.run('https://school.invalid', state(), signal, renew)
	]);
	assert.equal(calls, 1);
	const result = await b.run('https://school.invalid', state(), signal, renew);
	assert.equal(result.kind, 'renewed');
	assert.equal(calls, 1);
	assert.equal(
		[...shared.map.keys()].some((key) => key.endsWith(':lease')),
		false
	);
});

test('Web Lock callback checks shared deadlines before invoking renewal', async () => {
	const shared = sharedHarness();
	const origin = 'https://school.invalid';
	shared.storage.setItem(
		sessionActivityKeys(origin, sessionId).metadata,
		JSON.stringify(state(ACTIVITY_WINDOW_MS))
	);
	let locks = 0;
	const coordinator = createSessionActivityCoordinator({
		storage: shared.storage,
		now: () => start,
		random: () => 0,
		wait: async () => {},
		locks: {
			request: async (_name, _options, callback) => {
				locks++;
				return callback({ name: 'activity', mode: 'exclusive' });
			}
		}
	});
	let calls = 0;
	const result = await coordinator.run(origin, state(), new AbortController().signal, async () => {
		calls++;
		return { kind: 'retry' };
	});
	assert.equal(locks, 1);
	assert.equal(calls, 0);
	assert.equal(result.kind, 'renewed');
});

test('shared state rejects malformed data and is isolated by tenant origin and session', () => {
	assert.equal(readSharedSession('{"sessionId":"private-token"}'), null);
	assert.equal(readSharedSession('not json'), null);
	assert.notEqual(
		sessionActivityKeys('https://a.invalid', sessionId).lock,
		sessionActivityKeys('https://b.invalid', sessionId).lock
	);
	assert.notEqual(
		sessionActivityKeys('https://a.invalid', sessionId).lock,
		sessionActivityKeys('https://a.invalid', 'new').lock
	);
});
