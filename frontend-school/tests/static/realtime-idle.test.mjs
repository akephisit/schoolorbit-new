import assert from 'node:assert/strict';
import test from 'node:test';

async function harness() {
	const module = await import('../../src/lib/realtime/visibility-idle.ts').catch(() => ({}));
	assert.equal(
		typeof module.createVisibilityIdle,
		'function',
		'visibility idle lifecycle must exist'
	);
	let hidden = false;
	let timer;
	let listener;
	let pauses = 0;
	let resumes = 0;
	const idle = module.createVisibilityIdle({
		isHidden: () => hidden,
		setTimer: (callback, delay) => {
			assert.equal(delay, 60_000);
			timer = callback;
			return 1;
		},
		clearTimer: () => {
			timer = undefined;
		},
		addListener: (callback) => {
			listener = callback;
		},
		removeListener: () => {
			listener = undefined;
		},
		onPause: () => pauses++,
		onResume: () => resumes++
	});
	return {
		idle,
		hide() {
			hidden = true;
			listener?.();
		},
		show() {
			hidden = false;
			listener?.();
		},
		expire() {
			const callback = timer;
			timer = undefined;
			callback?.();
		},
		get pauses() {
			return pauses;
		},
		get resumes() {
			return resumes;
		},
		get listener() {
			return listener;
		},
		get timer() {
			return timer;
		}
	};
}

test('short hidden visits cancel the grace timer without reconnecting', async () => {
	const h = await harness();
	h.idle.start();
	h.hide();
	h.show();
	h.expire();
	assert.equal(h.pauses, 0);
	assert.equal(h.resumes, 0);
});
test('idle pauses once and visible resumes once, with complete disconnect cleanup', async () => {
	const h = await harness();
	h.idle.start();
	h.hide();
	h.expire();
	h.hide();
	h.expire();
	assert.equal(h.pauses, 1);
	assert.equal(h.idle.paused, true);
	h.show();
	h.show();
	assert.equal(h.resumes, 1);
	assert.equal(h.idle.paused, false);
	h.hide();
	h.idle.stop();
	h.expire();
	h.show();
	assert.equal(h.pauses, 1);
	assert.equal(h.listener, undefined);
	assert.equal(h.timer, undefined);
});
test('starting while hidden arms the grace period and repeated starts do not reset it', async () => {
	const h = await harness();
	h.hide();
	h.idle.start();
	const timer = h.timer;
	h.idle.start();
	assert.equal(h.timer, timer);
	h.expire();
	assert.equal(h.pauses, 1);
});
