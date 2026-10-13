import assert from 'node:assert/strict';
import test from 'node:test';
import {
	bangkokDateTime,
	occurredAt,
	resolveBehaviorOccurredAt
} from '../../src/lib/features/behavior/presentation.ts';
test('civil day defaults use Bangkok midnight and preserve seconds', () => {
	assert.deepEqual(bangkokDateTime(new Date('2026-10-13T17:34:56.789Z')), {
		date: '2026-10-14',
		time: '00:34:56'
	});
	assert.equal(occurredAt('2026-10-14', '10:34:56'), '2026-10-14T03:34:56.000Z');
	assert.equal(occurredAt('2026-10-14', '10:34'), '2026-10-14T03:34:00.000Z');
});
test('untouched default keeps milliseconds after an immediate reset cutoff', () => {
	const instant = '2026-10-14T03:34:56.789Z';
	assert.equal(resolveBehaviorOccurredAt('2026-10-14', '10:34:56', instant), instant);
	assert.equal(
		resolveBehaviorOccurredAt('2026-10-13', '10:34:56', instant),
		'2026-10-13T03:34:56.000Z'
	);
	assert.equal(
		resolveBehaviorOccurredAt('2026-10-14', '10:35:00', instant),
		'2026-10-14T03:35:00.000Z'
	);
});
