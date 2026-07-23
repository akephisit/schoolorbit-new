import assert from 'node:assert/strict';
import test from 'node:test';

import {
	createRequestCoordinator,
	isAbortError
} from '../../src/lib/utils/request-coordinator.ts';

function deferred() {
	let resolve;
	let reject;
	const promise = new Promise((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

test('same scope and key reuse one in-flight operation', async () => {
	const coordinator = createRequestCoordinator();
	const gate = deferred();
	let calls = 0;
	const operation = async () => {
		calls += 1;
		await gate.promise;
		return 7;
	};
	const first = coordinator.run('activity', 'semester-a', operation);
	const second = coordinator.run('activity', 'semester-a', operation);
	assert.strictEqual(first, second);
	gate.resolve();
	assert.equal(await second, 7);
	assert.equal(calls, 1);
});

test('a new key aborts the prior request without deleting the newer record', async () => {
	const coordinator = createRequestCoordinator();
	let firstSignal;
	const first = coordinator.run('activity', 'semester-a', (signal) => {
		firstSignal = signal;
		return new Promise((_resolve, reject) => {
			signal.addEventListener('abort', () => reject(signal.reason), { once: true });
		});
	});
	const second = coordinator.run('activity', 'semester-b', async () => 2);
	assert.equal(firstSignal?.aborted, true);
	await assert.rejects(first, (error) => isAbortError(error));
	assert.equal(await second, 2);
});

test('abortAll aborts every scope', async () => {
	const coordinator = createRequestCoordinator();
	const signals = [];
	const pending = ['activity', 'entries'].map((scope) =>
		coordinator.run(scope, 'semester-a', (signal) => {
			signals.push(signal);
			return new Promise((_resolve, reject) => {
				signal.addEventListener('abort', () => reject(signal.reason), { once: true });
			});
		})
	);
	coordinator.abortAll();
	assert.deepEqual(
		signals.map((signal) => signal.aborted),
		[true, true]
	);
	await Promise.all(pending.map((promise) => assert.rejects(promise, isAbortError)));
});
