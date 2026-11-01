import assert from 'node:assert/strict';
import test from 'node:test';
import type { AcademicTermChangeSet } from '../../src/lib/api/learning-delivery.js';
import { teacherChangeSelection } from '../../src/lib/academic/teacher-change-selection.ts';

type Selection = Pick<AcademicTermChangeSet, 'baseTeacherAssignments' | 'items'>;
const members: Selection['baseTeacherAssignments'] = [
	{
		assignmentId: 'same-day',
		learningGroupId: 'group',
		teacherId: 'teacher-a',
		displayName: 'ครูต้นทาง',
		role: 'primary'
	},
	{
		assignmentId: 'future-ended',
		learningGroupId: 'group',
		teacherId: 'teacher-b',
		displayName: 'ครูอีกคน',
		role: 'secondary'
	},
	{
		assignmentId: 'other-group',
		learningGroupId: 'other',
		teacherId: 'teacher-c',
		displayName: 'ครูต่างกลุ่ม',
		role: 'assistant'
	}
];
function command(
	actionKind: 'stop_group_teacher' | 'adjust_group_teacher_role' | 'add_group_teacher'
): Selection['items'][number] {
	const common = {
		id: actionKind,
		learningGroupId: 'group',
		learningGroupLabel: 'กลุ่ม',
		teacherId: 'teacher-a',
		teacherLabel: 'ครูต้นทาง',
		rowVersion: 1,
		createdAt: '2026-11-01T00:00:00Z',
		updatedAt: '2026-11-01T00:00:00Z',
		createdBy: 'actor'
	};
	return actionKind === 'add_group_teacher'
		? { ...common, actionKind, teacherRole: 'primary' }
		: { ...common, actionKind, learningGroupTeacherId: 'same-day', teacherRole: 'secondary' };
}

test('teacher choices come from the exact base group membership without date eligibility guesses', () => {
	const state = teacherChangeSelection({ baseTeacherAssignments: members, items: [] }, 'group');
	assert.deepEqual(
		state.availableAssignments.map((item) => item.assignmentId),
		['same-day', 'future-ended']
	);
	assert.deepEqual([...state.blockedTeacherIds], ['teacher-a', 'teacher-b']);
	assert.deepEqual(
		teacherChangeSelection({ baseTeacherAssignments: [], items: [] }, 'group').availableAssignments,
		[]
	);
});

test('a pending Stop allows the same teacher to be added again while pending Adjust retains membership', () => {
	const stopped = teacherChangeSelection(
		{ baseTeacherAssignments: members, items: [command('stop_group_teacher')] },
		'group'
	);
	assert.deepEqual(
		stopped.availableAssignments.map((item) => item.assignmentId),
		['future-ended']
	);
	assert.equal(stopped.blockedTeacherIds.has('teacher-a'), false);
	const adjusted = teacherChangeSelection(
		{ baseTeacherAssignments: members, items: [command('adjust_group_teacher_role')] },
		'group'
	);
	assert.deepEqual(
		adjusted.availableAssignments.map((item) => item.assignmentId),
		['future-ended']
	);
	assert.equal(adjusted.blockedTeacherIds.has('teacher-a'), true);
});

test('pending additions still block duplicate teachers after Stop and other-group actions stay scoped', () => {
	const state = teacherChangeSelection(
		{
			baseTeacherAssignments: members,
			items: [command('stop_group_teacher'), command('add_group_teacher')]
		},
		'group'
	);
	assert.equal(state.blockedTeacherIds.has('teacher-a'), true);
	const other = { ...command('stop_group_teacher'), learningGroupId: 'other' };
	const scoped = teacherChangeSelection(
		{ baseTeacherAssignments: members, items: [other] },
		'group'
	);
	assert.equal(scoped.availableAssignments.length, 2);
	assert.equal(scoped.blockedTeacherIds.has('teacher-a'), true);
});
