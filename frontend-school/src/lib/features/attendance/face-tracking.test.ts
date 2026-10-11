import { test } from 'node:test';
import assert from 'node:assert/strict';
import { FaceTracker, enrollmentPose } from './face-tracking.ts';
const a = { studentId: 'a', box: { x: 0.1, y: 0.1, width: 0.15, height: 0.2 } };
const b = { studentId: 'b', box: { x: 0.6, y: 0.1, width: 0.15, height: 0.2 } };
test('independent people need three consecutive observations', () => {
	const tracker = new FaceTracker();
	assert.equal(tracker.observe([a, b], 0).size, 0);
	assert.equal(tracker.observe([a, b], 200).size, 0);
	assert.deepEqual([...tracker.observe([a, b], 400)], ['a', 'b']);
});
test('duplicate identities, absence, elapsed frames and position jumps reset confirmation', () => {
	for (const disruption of [[a, a], [], [{ ...a, box: b.box }]]) {
		const tracker = new FaceTracker();
		tracker.observe([a], 0);
		tracker.observe([a], 200);
		assert.equal(tracker.observe(disruption, 400).size, 0);
		assert.equal(tracker.observe([a], 600).size, 0);
	}
	const tracker = new FaceTracker();
	tracker.observe([a], 0);
	tracker.observe([a], 200);
	assert.equal(tracker.observe([a], 3000).size, 0);
	tracker.clear();
	assert.equal(tracker.observe([a], 3100).size, 0);
});
test('unmirrored enrollment requires center then distinct anatomical left and right poses', () => {
	assert.equal(enrollmentPose(0, 0), true);
	assert.equal(enrollmentPose(0.12, 0), false);
	assert.equal(enrollmentPose(0.12, 1), true);
	assert.equal(enrollmentPose(-0.12, 1), false);
	assert.equal(enrollmentPose(-0.12, 2), true);
	assert.equal(enrollmentPose(0.12, 2), false);
});

test('matching preserves distance and ambiguity margins while choosing nearest two scores', async () => {
	const { matchFace } = await import('./face-camera.ts');
	const config = {
		enabled: true,
		weekdays: [1],
		lateAfter: '08:00:00',
		digestTimes: [],
		evidenceDays: 30,
		faceDistance: 0.45,
		faceMargin: 0.1,
		activityCountsAsPresent: true
	};
	const face = (studentId: string, value: number) => ({
		studentId,
		model: 'fixture',
		rowVersion: 1,
		descriptors: [{ values: Array(128).fill(value) }]
	});
	const descriptor = Array(128).fill(0.01);
	assert.equal(matchFace([face('a', 0.01), face('b', 0.5)], descriptor, config), 'a');
	assert.equal(matchFace([face('a', 0.01), face('b', 0.011)], descriptor, config), null);
	assert.equal(matchFace([face('a', 0.5)], descriptor, config), null);
	assert.equal(matchFace([face('a', 0.01)], Array(128).fill(NaN), config), null);
});
