import assert from 'node:assert/strict';
import test from 'node:test';
import { correspondenceUuidV7 } from '../../src/lib/correspondence/uuid-v7.ts';

test('new draft IDs encode the millisecond timestamp and UUIDv7 version/variant', () => {
	const now = Date.UTC(2026, 9, 18, 9, 30);
	const first = correspondenceUuidV7(now);
	assert.match(first, /^[\da-f]{8}-[\da-f]{4}-7[\da-f]{3}-[89ab][\da-f]{3}-[\da-f]{12}$/);
	assert.equal(Number.parseInt(first.replaceAll('-', '').slice(0, 12), 16), now);
	assert.notEqual(correspondenceUuidV7(now), first);
	assert.equal(Number.parseInt(correspondenceUuidV7(0).replaceAll('-', '').slice(0, 12), 16), 0);
	for (const invalid of [-1, Number.NaN, 1.5, 0x1000000000000])
		assert.throws(() => correspondenceUuidV7(invalid));
});
