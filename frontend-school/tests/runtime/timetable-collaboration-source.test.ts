import assert from 'node:assert/strict';
import test from 'node:test';
import {
	placementSourceKey,
	sourceGhost
} from '../../src/lib/academic/timetable/collaboration-source.ts';
import {
	makeTimetableBlock,
	makeStructuralTimetableBlock,
	timetableIds
} from '../e2e/timetable-test-harness.ts';
import { makeSortingFixture } from '../e2e/timetable-sorting-fixtures.ts';

test('a source lock keeps the same stable identity across resource views and row versions', () => {
	assert.equal(
		placementSourceKey({ kind: 'existing_block', blockId: timetableIds.blockA, rowVersion: 1 }),
		placementSourceKey({ kind: 'existing_block', blockId: timetableIds.blockA, rowVersion: 2 })
	);
	assert.notEqual(
		placementSourceKey({
			kind: 'ordinary_demand',
			learningGroupId: timetableIds.groupA,
			learningOfferingId: timetableIds.offeringA
		}),
		placementSourceKey({
			kind: 'ordinary_demand',
			learningGroupId: timetableIds.groupB,
			learningOfferingId: timetableIds.offeringA
		})
	);
});

test('a remote ghost resolves only authorized title and course code from the recipient workspace', () => {
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1, {
		roomId: timetableIds.roomB
	});
	const workspace = { ...makeSortingFixture(), blocks: [block] };
	assert.deepEqual(
		sourceGhost(workspace, { kind: 'existing_block', blockId: block.id, rowVersion: 1 }),
		{
			code: 'ค21101',
			title: 'คณิตศาสตร์พื้นฐาน'
		}
	);
	assert.equal(
		sourceGhost(workspace, { kind: 'existing_block', blockId: 'hidden-block', rowVersion: 1 }),
		null
	);
	workspace.blocks = [
		makeStructuralTimetableBlock(timetableIds.blockB, timetableIds.period2, 'พัก')
	];
	assert.deepEqual(
		sourceGhost(workspace, { kind: 'existing_block', blockId: timetableIds.blockB, rowVersion: 1 }),
		{ code: null, title: 'พัก' }
	);
});
