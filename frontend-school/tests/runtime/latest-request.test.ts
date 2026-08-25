import assert from 'node:assert/strict';
import test from 'node:test';

import { LatestRequest, isAbortError } from '../../src/lib/async/latest-request.ts';

test('begin aborts the prior request and advances the current revision', () => {
	const owner = new LatestRequest();
	const first = owner.begin();
	const second = owner.begin();

	assert.equal(first.signal.aborted, true);
	assert.equal(second.signal.aborted, false);
	assert.equal(owner.isCurrent(first.revision), false);
	assert.equal(owner.isCurrent(second.revision), true);
});

test('abort invalidates the active request and abort errors are narrowed', () => {
	const owner = new LatestRequest();
	const active = owner.begin();

	owner.abort();

	assert.equal(active.signal.aborted, true);
	assert.equal(owner.isCurrent(active.revision), false);
	assert.equal(isAbortError(new DOMException('aborted', 'AbortError')), true);
	assert.equal(isAbortError(Object.assign(new Error('aborted'), { name: 'AbortError' })), true);
	assert.equal(isAbortError(new Error('network')), false);
});
