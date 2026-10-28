import assert from 'node:assert/strict';
import test from 'node:test';
import {
	buildTimetableLessonColors,
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

import {
	makeSortingFixture,
	makeColorCohort,
	sortingIds
} from '../e2e/timetable-sorting-fixtures.ts';
const colors = buildTimetableLessonColors(makeSortingFixture());

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
		timetableLessonColorStyle(timetableBlockColorKey(after), colors),
		timetableLessonColorStyle(timetableBlockColorKey(before), colors)
	);
	assert.equal(
		timetablePlacementColorKey(
			{ kind: 'existing_block', blockId: after.id, rowVersion: after.rowVersion },
			after
		),
		timetableBlockColorKey(before)
	);
});

test('cooccurring subject/class groups and synchronized activities have distinct paired colors', () => {
	const keys = [
		ordinaryLessonColorKey(timetableIds.groupA),
		ordinaryLessonColorKey(timetableIds.groupB),
		ordinaryLessonColorKey(sortingIds.activity),
		synchronizedLessonColorKey(timetableIds.offeringSync)
	];
	const styles = keys.map((key) => timetableLessonColorStyle(key, colors));
	assert.equal(new Set(styles).size, keys.length);
	assert.notEqual(ordinaryLessonColorKey('same-id'), synchronizedLessonColorKey('same-id'));
});

test('structural periods and unresolved drag blocks stay neutral', () => {
	const block = makeStructuralTimetableBlock(timetableIds.blockA, timetableIds.period1, 'พัก');
	assert.equal(timetableBlockColorKey(block), null);
	assert.equal(timetableLessonColorStyle(null, colors), undefined);
	assert.equal(
		timetablePlacementColorKey({
			kind: 'existing_block',
			blockId: block.id,
			rowVersion: block.rowVersion
		}),
		null
	);
});

test('sixteen formerly clustered colors separate when lessons share a teacher or class', () => {
	for (const relation of ['teacher', 'homeroom']) {
		const groups = makeColorCohort().map((group) => ({
			...group,
			homeroomIds: relation === 'homeroom' ? group.homeroomIds : [],
			eligibleInstructors: relation === 'teacher' ? group.eligibleInstructors : []
		}));
		const assignments = buildTimetableLessonColors({
			learningGroups: groups,
			synchronizedDemands: []
		});
		assert.equal(assignments.size, 16);
		assert.equal(new Set([...assignments.values()].map((tone) => tone.light)).size, 16);
		assert.equal(new Set([...assignments.values()].map((tone) => tone.dark)).size, 16);
	}
});

test('allocation ignores source ordering and remaining periods while preserving immutable source data', () => {
	const source = makeSortingFixture(),
		before = structuredClone(source);
	const expected = buildTimetableLessonColors(source);
	const reordered = buildTimetableLessonColors({
		learningGroups: [...source.learningGroups].reverse().map((group) => ({
			...group,
			homeroomIds: [...group.homeroomIds].reverse(),
			eligibleInstructors: [...group.eligibleInstructors].reverse()
		})),
		synchronizedDemands: [...source.synchronizedDemands]
			.reverse()
			.map((demand) => ({ ...demand, scheduledPeriods: demand.requiredPeriods }))
	});
	assert.deepEqual(reordered, expected);
	assert.deepEqual(source, before);
});

test('linked synchronized groups contribute teachers without consuming invisible ordinary color slots', () => {
	const groups = makeColorCohort(15);
	const linked = Array.from({ length: 10 }, (_, index) => ({
		...groups[0],
		id: `linked-${index}`,
		learningOfferingId: timetableIds.offeringSync
	}));
	const assignments = buildTimetableLessonColors({
		learningGroups: [...groups, ...linked],
		synchronizedDemands: [
			{
				learningOfferingId: timetableIds.offeringSync,
				offeringCode: 'CLUB',
				offeringName: 'ชุมนุม',
				intendedHomeroomIds: [],
				conflictGroupCount: 0,
				excludedGroupCount: 0,
				linkedGroupCount: 0,
				pendingGroupCount: 0,
				requiredPeriods: 1,
				scheduledPeriods: 0
			}
		]
	});
	assert.equal(assignments.size, 16);
	assert.equal(new Set([...assignments.values()].map((tone) => tone.light)).size, 16);
	assert.ok(assignments.has(synchronizedLessonColorKey(timetableIds.offeringSync)));
	for (const group of linked)
		assert.equal(assignments.has(ordinaryLessonColorKey(group.id)), false);
});

test('a two-thousand-lesson shared cohort reuses the finite palette evenly after it is exhausted', () => {
	const assignments = buildTimetableLessonColors({
		learningGroups: makeColorCohort(2000),
		synchronizedDemands: []
	});
	const usage = new Map<string, number>();
	for (const tone of assignments.values()) usage.set(tone.light, (usage.get(tone.light) ?? 0) + 1);
	assert.equal(assignments.size, 2000);
	assert.equal(usage.size, 24);
	assert.ok(Math.max(...usage.values()) - Math.min(...usage.values()) <= 1);
});
