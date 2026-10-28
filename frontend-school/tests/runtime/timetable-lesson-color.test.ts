import assert from 'node:assert/strict';
import test from 'node:test';
import {
	ordinaryLessonColorKey,
	synchronizedLessonColorKey,
	timetableBlockColorKey,
	timetablePlacementColorKey,
	timetableLessonColorStyle
} from '../../src/lib/academic/timetable/lesson-color.ts';
import {
	makeStructuralTimetableBlock,
	makeSynchronizedTimetableBlock,
	makeTimetableBlock,
	timetableIds
} from '../e2e/timetable-test-harness.ts';

test('ordinary demand, pending block and persisted lesson share their learning group color', () => {
	const expected = ordinaryLessonColorKey(timetableIds.groupA);
	assert.equal(
		timetablePlacementColorKey({
			kind: 'ordinary_demand',
			learningGroupId: timetableIds.groupA,
			learningOfferingId: timetableIds.offeringA
		}),
		expected
	);
	for (const id of ['optimistic:temporary', timetableIds.createdBlock]) {
		const block = makeTimetableBlock(id, timetableIds.period1);
		assert.equal(timetableBlockColorKey(block), expected);
	}
});

test('ordinary learner activities retain group colors even without visible course codes', () => {
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	block.blockKind = 'activity';
	block.schedulingMode = 'independent';
	block.offeringCode = null;
	assert.equal(timetableBlockColorKey(block), ordinaryLessonColorKey(timetableIds.groupA));
});

test('synchronized activities keep offering colors when groups and teacher targets change', () => {
	const expected = synchronizedLessonColorKey(timetableIds.offeringSync);
	assert.equal(
		timetablePlacementColorKey({
			kind: 'synchronized_offering',
			learningOfferingId: timetableIds.offeringSync
		}),
		expected
	);
	for (const block of [
		makeSynchronizedTimetableBlock(timetableIds.blockA, timetableIds.period1),
		makeSynchronizedTimetableBlock(
			timetableIds.createdBlock,
			timetableIds.period3,
			[timetableIds.teacherB],
			[timetableIds.teacherA]
		)
	])
		assert.equal(timetableBlockColorKey(block), expected);
});

test('moving and reloading existing blocks does not depend on slot, names or block IDs', () => {
	const before = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	const after = makeTimetableBlock(timetableIds.createdBlock, timetableIds.period3, {
		name: 'เปลี่ยนชื่อวิชา',
		instructorIds: [timetableIds.teacherB]
	});
	after.dayOfWeek = 'FRI';
	assert.equal(
		timetableLessonColorStyle(timetableBlockColorKey(after)),
		timetableLessonColorStyle(timetableBlockColorKey(before))
	);
	assert.equal(
		timetablePlacementColorKey(
			{ kind: 'existing_block', blockId: after.id, rowVersion: after.rowVersion },
			after
		),
		timetableBlockColorKey(before)
	);
});

test('different subject/class groups and synchronized activities have distinct continuous colors', () => {
	const keys = [
		ordinaryLessonColorKey(timetableIds.groupA),
		ordinaryLessonColorKey(timetableIds.groupB),
		ordinaryLessonColorKey('71000000-0000-4000-8000-000000000203'),
		synchronizedLessonColorKey(timetableIds.offeringSync)
	];
	const colors = keys.map(timetableLessonColorStyle);
	assert.equal(new Set(colors).size, keys.length);
	assert.notEqual(
		timetableLessonColorStyle(ordinaryLessonColorKey('same-id')),
		timetableLessonColorStyle(synchronizedLessonColorKey('same-id'))
	);
});

test('structural periods and unresolved drag blocks stay neutral', () => {
	const block = makeStructuralTimetableBlock(timetableIds.blockA, timetableIds.period1, 'พัก');
	assert.equal(timetableBlockColorKey(block), null);
	assert.equal(timetableLessonColorStyle(null), undefined);
	assert.equal(
		timetablePlacementColorKey({
			kind: 'existing_block',
			blockId: block.id,
			rowVersion: block.rowVersion
		}),
		null
	);
});
