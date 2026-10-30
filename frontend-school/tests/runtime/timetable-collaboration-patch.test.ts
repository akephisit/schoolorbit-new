import assert from 'node:assert/strict';
import test from 'node:test';
import type { DraftPatch } from '../../src/lib/api/timetable.js';
import { applyDraftPatch } from '../../src/lib/academic/timetable/collaboration-patch.ts';
import { collaborationWorkspace } from './timetable-collaboration-fixture.ts';
import { makeTimetableBlock, timetableIds } from '../e2e/timetable-test-harness.ts';

test('a partial patch changes only affected blocks/demands and preserves server totals beyond visible scope', () => {
	const workspace = collaborationWorkspace();
	workspace.blocks = [
		makeTimetableBlock(timetableIds.blockA, timetableIds.period1),
		makeTimetableBlock(timetableIds.blockB, timetableIds.period2)
	];
	const first = workspace.ordinaryDemands[0];
	const unchanged = workspace.ordinaryDemands[1];
	const moved = {
		...workspace.blocks[0],
		bellSchedulePeriodId: timetableIds.period3,
		rowVersion: 2
	};
	const patch: DraftPatch = {
		timetableVersionId: workspace.version.id,
		mutationId: 'remote-change',
		upsertedBlocks: [moved],
		removedBlockIds: [timetableIds.blockB],
		ordinaryDemands: [{ ...first, scheduledPeriods: 7, remainingPeriods: 3 }],
		synchronizedDemands: [],
		summary: { ...workspace.summary, blockCount: 17 },
		sourceIssues: [],
		totalDraftBlockCount: 17,
		version: { ...workspace.version, rowVersion: 4 }
	};
	const next = applyDraftPatch(workspace, patch);
	assert.deepEqual(next.blocks, [moved]);
	assert.equal(next.ordinaryDemands[0].scheduledPeriods, 7);
	assert.equal(next.ordinaryDemands[0].remainingPeriods, 3);
	assert.equal(next.ordinaryDemands[1], unchanged);
	assert.equal(next.summary.blockCount, 17);
	assert.equal(next.totalDraftBlockCount, 17);
	assert.equal(next.version.rowVersion, 4);
	assert.equal(workspace.blocks.length, 2);
	assert.equal(
		applyDraftPatch(workspace, { ...patch, timetableVersionId: 'another-draft' }),
		workspace
	);
});
