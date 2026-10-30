import assert from 'node:assert/strict';
import test from 'node:test';
import { collaborationContextsMatch } from '../../src/lib/academic/timetable/collaboration-view.ts';
import type { DraftContext } from '../../src/lib/api/timetable.js';
import { timetableIds } from '../e2e/timetable-test-harness.ts';

for (const viewMode of ['homeroom', 'teacher', 'learning_group'] as const) {
	test(`${viewMode} pointers require the same non-null selected owner and matching view mode`, () => {
		const local: DraftContext = { viewMode, viewId: timetableIds.teacherA };
		assert.equal(collaborationContextsMatch(local, { ...local }), true);
		assert.equal(
			collaborationContextsMatch(local, { ...local, viewId: timetableIds.teacherB }),
			false
		);
		assert.equal(collaborationContextsMatch(local, { ...local, viewId: null }), false);
		assert.equal(
			collaborationContextsMatch({ ...local, viewId: null }, { ...local, viewId: null }),
			false
		);
		for (const remoteMode of ['homeroom', 'teacher', 'learning_group'] as const) {
			assert.equal(
				collaborationContextsMatch(local, { viewMode: remoteMode, viewId: local.viewId }),
				viewMode === remoteMode
			);
		}
	});
}

test('whole-school cursors use the remote row and selected day independently of the local hovered row', () => {
	const local: DraftContext = {
		viewMode: 'wholeSchool',
		viewId: timetableIds.teacherA,
		dayOfWeek: 'MON'
	};
	assert.equal(
		collaborationContextsMatch(local, { ...local, viewId: timetableIds.teacherB }),
		true
	);
	assert.equal(
		collaborationContextsMatch(
			{ ...local, viewId: null },
			{ ...local, viewId: timetableIds.teacherB }
		),
		true
	);
	assert.equal(collaborationContextsMatch(local, { ...local, viewId: null }), false);
	assert.equal(collaborationContextsMatch(local, { ...local, dayOfWeek: 'TUE' }), false);
	assert.equal(collaborationContextsMatch(local, { ...local, dayOfWeek: null }), false);
	assert.equal(
		collaborationContextsMatch(local, { viewMode: 'teacher', viewId: timetableIds.teacherA }),
		false
	);
});

test('the homeroom and whole-school bridge requires the corresponding row and an explicit overview day', () => {
	const homeroom: DraftContext = { viewMode: 'homeroom', viewId: timetableIds.homeroom };
	const overview: DraftContext = {
		viewMode: 'wholeSchool',
		viewId: timetableIds.homeroom,
		dayOfWeek: 'TUE'
	};
	assert.equal(collaborationContextsMatch(homeroom, overview), true);
	assert.equal(collaborationContextsMatch(overview, homeroom), true);
	assert.equal(
		collaborationContextsMatch(homeroom, { ...overview, viewId: timetableIds.teacherA }),
		false
	);
	assert.equal(collaborationContextsMatch(homeroom, { ...overview, dayOfWeek: null }), false);
	assert.equal(collaborationContextsMatch({ ...homeroom, viewMode: 'teacher' }, overview), false);
});
