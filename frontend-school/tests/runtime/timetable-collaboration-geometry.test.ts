import assert from 'node:assert/strict';
import test from 'node:test';
import {
	containsPointer,
	createPointerCoalescer,
	interpolatePointer,
	intersectRects,
	projectPointer,
	relativePointer
} from '../../src/lib/academic/timetable/collaboration-geometry.ts';

test('semantic coordinates reproject into a differently sized and scrolled recipient cell', () => {
	const relative = relativePointer(
		{ x: 220, y: 145 },
		{ left: 100, top: 120, width: 160, height: 100 }
	);
	assert.deepEqual(relative, { u: 0.75, v: 0.25 });
	assert.deepEqual(projectPointer(relative!, { left: 24, top: -40, width: 320, height: 200 }), {
		x: 264,
		y: 10
	});
	assert.deepEqual(projectPointer(relative!, { left: 84, top: 90, width: 80, height: 50 }), {
		x: 144,
		y: 102.5
	});
});

test('hidden or invalid geometry cannot produce a cursor and edge coordinates stay bounded', () => {
	const rect = { left: 10, top: 20, width: 100, height: 80 };
	assert.deepEqual(relativePointer({ x: -20, y: 200 }, rect), { u: 0, v: 1 });
	assert.deepEqual(projectPointer({ u: 3, v: -4 }, rect), { x: 110, y: 20 });
	assert.equal(relativePointer({ x: NaN, y: 20 }, rect), null);
	assert.equal(relativePointer({ x: 10, y: 20 }, { ...rect, width: 0 }), null);
	assert.equal(projectPointer({ u: 0.5, v: Infinity }, rect), null);
});

test('interpolation uses normalized positions so resizing never reuses old screen pixels', () => {
	const position = interpolatePointer({ u: 0, v: 1 }, { u: 1, v: 0 }, 33);
	assert.deepEqual(position, { u: 0.5, v: 0.5 });
	assert.deepEqual(projectPointer(position, { left: 80, top: 40, width: 240, height: 120 }), {
		x: 200,
		y: 100
	});
	assert.deepEqual(interpolatePointer({ u: 0, v: 1 }, { u: 1, v: 0 }, 100), { u: 1, v: 0 });
	assert.deepEqual(interpolatePointer({ u: 0, v: 1 }, { u: 1, v: 0 }, -10), { u: 0, v: 1 });
});

test('nested clipping hides a point scrolled behind the board boundary', () => {
	const visible = intersectRects(
		{ left: -50, top: 100, width: 200, height: 100 },
		{ left: 20, top: 50, width: 400, height: 500 }
	);
	assert.deepEqual(visible, { left: 20, top: 100, width: 130, height: 100 });
	assert.equal(containsPointer(visible!, { x: 0, y: 140 }), false);
	assert.equal(containsPointer(visible!, { x: 60, y: 140 }), true);
	assert.equal(intersectRects(visible!, { left: 500, top: 500, width: 50, height: 50 }), null);
});

test('cursor samples coalesce to 25Hz latest values and disposal cancels queued work', () => {
	let callback: (() => void) | null = null;
	let scheduled = 0;
	let cleared = 0;
	const sent: number[] = [];
	const coalescer = createPointerCoalescer<number, number>({
		send: (value) => sent.push(value),
		setTimer: (next, delay) => {
			assert.equal(delay, 40);
			callback = next;
			return ++scheduled;
		},
		clearTimer: () => {
			cleared++;
		}
	});
	for (let index = 0; index < 100; index++) coalescer.push(index);
	assert.equal(scheduled, 1);
	callback!();
	assert.deepEqual(sent, [99]);
	coalescer.push(100);
	coalescer.cancel();
	callback!();
	assert.deepEqual(sent, [99]);
	coalescer.push(101);
	coalescer.dispose();
	callback!();
	coalescer.push(102);
	assert.deepEqual(sent, [99]);
	assert.equal(scheduled, 3);
	assert.ok(cleared >= 3);
});
