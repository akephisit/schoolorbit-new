import test from 'node:test';
import assert from 'node:assert/strict';
import {
	resizeWithinPage,
	fromDisplay,
	pointerPoint
} from '../../src/lib/correspondence/geometry.ts';

test('pointer and keyboard resize preserve valid tiny placements near every page edge', () => {
	for (const placement of [
		{ x: 0.99, y: 0.99, width: 0.01, height: 0.01 },
		{ x: 0.999, y: 0.2, width: 0.001, height: 0.05 },
		{ x: 0.2, y: 0.999, width: 0.05, height: 0.001 }
	]) {
		for (const rotation of [0, 90, 180, 270]) {
			const point = pointerPoint(
				999,
				999,
				{ left: 0, top: 0, width: 1000, height: 1000 },
				rotation
			);
			const keyboard = fromDisplay({ x: 0.51, y: 0.51 }, rotation);
			for (const size of [
				{ width: point.x - placement.x, height: point.y - placement.y },
				{ width: placement.width + keyboard.x - 0.5, height: placement.height + keyboard.y - 0.5 }
			]) {
				const resized = resizeWithinPage(placement, size.width, size.height);
				assert.ok(resized.width > 0 && resized.height > 0);
				assert.ok(resized.x + resized.width <= 1);
				assert.ok(resized.y + resized.height <= 1);
				assert.equal(resized.x, placement.x);
				assert.equal(resized.y, placement.y);
			}
		}
	}
});
