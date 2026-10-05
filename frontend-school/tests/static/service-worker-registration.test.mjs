import assert from 'node:assert/strict';
import test from 'node:test';
import { createServiceWorkerRegistrationOwner } from '../../src/lib/pwa/service-worker-registration.ts';

test('startup and push share one module registration and its readiness', async () => {
	const registration = { scope: 'https://school.test/' };
	let calls = 0;
	const container = {
		ready: Promise.resolve(registration),
		register: async (url, options) => {
			calls++;
			assert.equal(url, '/service-worker.js');
			assert.deepEqual(options, { type: 'module', scope: '/', updateViaCache: 'none' });
			return registration;
		}
	};
	const get = createServiceWorkerRegistrationOwner(container);
	const first = get();
	assert.equal(get(), first);
	assert.equal(await first, registration);
	assert.equal(await get(), registration);
	assert.equal(calls, 1);
});

test('a failed registration can be retried without removing subscriptions', async () => {
	let calls = 0;
	const registration = {};
	const get = createServiceWorkerRegistrationOwner({
		ready: Promise.resolve(registration),
		register: async () => {
			if (++calls === 1) throw new Error('network offline');
			return registration;
		}
	});
	await assert.rejects(get(), /network offline/);
	assert.equal(await get(), registration);
	assert.equal(calls, 2);
});

test('stalled readiness times out and a later attempt can become ready', async () => {
	let ready = new Promise(() => {});
	let calls = 0;
	const registration = {};
	const get = createServiceWorkerRegistrationOwner(
		{
			get ready() {
				return ready;
			},
			register: async () => {
				calls++;
				return registration;
			}
		},
		15
	);
	await assert.rejects(get(), /ลองอีกครั้ง/);
	ready = Promise.resolve(registration);
	assert.equal(await get(), registration);
	assert.equal(calls, 2);
});
