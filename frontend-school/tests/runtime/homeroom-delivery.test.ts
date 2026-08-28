import assert from 'node:assert/strict';
import test from 'node:test';

import {
	filterHomeroomDeliveryRooms,
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
			curriculumId: 'curriculum-1',
			curriculumName: 'หลักสูตร 2569'
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
			curriculumId: 'curriculum-1',
			curriculumName: 'หลักสูตร 2569'
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
