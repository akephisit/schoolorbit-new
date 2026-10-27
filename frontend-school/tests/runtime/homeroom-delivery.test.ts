import assert from 'node:assert/strict';
import test from 'node:test';

import {
	filterHomeroomDeliveryRooms,
	sortHomeroomDeliveryItems,
	summarizeHomeroomDelivery
} from '../../src/lib/academic/homeroom-delivery.ts';

const rooms = [
	{
		homeroom: { id: 'room-1', name: 'ม.1/1' },
		gradeLevel: {
			id: 'grade-1',
			code: 'M1',
			name: 'มัธยมศึกษาปีที่ 1',
			levelType: 'secondary',
			levelOrder: 301
		},
		studyProgram: {
			id: 'program-1',
			code: 'SCI',
			name: 'วิทย์-คณิต',
			editionId: 'curriculum-1',
			levelName: 'หลักสูตร 2569'
		},
		expectedCount: 2,
		readyCount: 1,
		blockers: [],
		items: []
	},
	{
		homeroom: { id: 'room-2', name: 'ม.1/2' },
		gradeLevel: {
			id: 'grade-1',
			code: 'M1',
			name: 'มัธยมศึกษาปีที่ 1',
			levelType: 'secondary',
			levelOrder: 301
		},
		studyProgram: {
			id: 'program-2',
			code: 'ART',
			name: 'ศิลป์ภาษา',
			editionId: 'curriculum-1',
			levelName: 'หลักสูตร 2569'
		},
		expectedCount: 1,
		readyCount: 1,
		blockers: [],
		items: []
	}
] as const;

test('filters rooms by Thai room or program text and readiness', () => {
	assert.deepEqual(
		filterHomeroomDeliveryRooms(rooms, 'วิทย์', 'all').map((room) => room.homeroom.id),
		['room-1']
	);
	assert.deepEqual(
		filterHomeroomDeliveryRooms(rooms, '', 'attention').map((room) => room.homeroom.id),
		['room-1']
	);
});

test('summarizes readiness without treating missing work as ready', () => {
	assert.deepEqual(summarizeHomeroomDelivery(rooms), {
		roomCount: 2,
		expectedCount: 3,
		readyCount: 2,
		attentionRoomCount: 1
	});
});

test('room items use pinned BASIC then ADDITIONAL classification independently of required/elective and code digits', () => {
	const course = (
		requirementId: string,
		code: string,
		subjectType: string | null,
		requirementKind = 'required'
	) => ({
		requirementId,
		code,
		name: requirementId,
		subjectType,
		requirementKind,
		resourceKind: 'course' as const
	});
	const items = [
		{
			...course('activity-b', 'AA000', null),
			resourceKind: 'activity' as const,
			name: 'กิจกรรม ข'
		},
		course('additional', 'ค21101', 'ADDITIONAL'),
		course('legacy', 'ค11101', 'LEGACY-UNKNOWN'),
		course('basic-23', 'ค23101', 'BASIC', 'elective'),
		{
			...course('activity-a', 'ZZ000', null),
			resourceKind: 'activity' as const,
			name: 'กิจกรรม ก'
		},
		course('basic-21', 'ค21102', 'BASIC', 'elective'),
		course('unknown', 'ค01101', null)
	];
	const original = [...items];
	assert.deepEqual(
		sortHomeroomDeliveryItems(items).map((item) => item.requirementId),
		['basic-21', 'basic-23', 'additional', 'unknown', 'legacy', 'activity-a', 'activity-b']
	);
	assert.deepEqual(items, original);
});
