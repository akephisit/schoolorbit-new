import assert from 'node:assert/strict';
import test from 'node:test';
import {
	fromDisplay,
	toDisplay,
	pointerPoint,
	hasInk
} from '../../src/lib/correspondence/geometry.ts';

test('normalized page points round trip through every quarter rotation', () => {
	for (const rotation of [0, 90, 180, 270]) {
		const original = { x: 0.23, y: 0.71 };
		const roundTrip = fromDisplay(toDisplay(original, rotation), rotation);
		assert.ok(Math.abs(roundTrip.x - original.x) < 1e-12);
		assert.ok(Math.abs(roundTrip.y - original.y) < 1e-12);
	}
});
test('pointer conversion scales, clamps and unrotates the displayed page', () => {
	assert.deepEqual(pointerPoint(150, 100, { left: 50, top: 50, width: 200, height: 100 }), {
		x: 0.5,
		y: 0.5
	});
	assert.deepEqual(pointerPoint(-100, 200, { left: 0, top: 0, width: 100, height: 100 }, 90), {
		x: 1,
		y: 1
	});
});
test('empty and accidental taps cannot confirm a drawn signature', () => {
	assert.equal(hasInk([]), false);
	assert.equal(hasInk([{ points: [{ x: 0.2, y: 0.4 }] }]), false);
	assert.equal(
		hasInk([
			{
				points: [
					{ x: 0.2, y: 0.4 },
					{ x: 0.3, y: 0.5 }
				]
			}
		]),
		true
	);
});
