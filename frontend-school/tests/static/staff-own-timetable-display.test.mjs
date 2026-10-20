import assert from 'node:assert/strict';
import test from 'node:test';
import {
	buildStaffOwnTimetableBlockDisplay,
	buildTimetableBlockDisplay
} from '../../src/lib/academic/timetable/block-display.ts';

const group = (overrides = {}) => ({
	name: 'กิจกรรม · กลุ่มการเรียนรู้พิเศษ',
	homeroomNames: ['ม.4/1'],
	homeroomIds: ['class-1'],
	instructors: [{ teacherId: 'staff-1', displayName: 'ครูหนึ่ง' }],
	roomId: 'room-1',
	roomName: 'ห้องวิทยาศาสตร์',
	...overrides
});
const block = (overrides = {}) => ({
	blockKind: 'course',
	schedulingMode: null,
	groups: [group()],
	homerooms: [],
	teachers: [],
	...overrides
});

test('staff own labels use actual classes and physical rooms without learning group aliases', () => {
	assert.deepEqual(
		buildStaffOwnTimetableBlockDisplay(
			block({
				homerooms: [
					{ name: 'ม.4/1', roomId: 'room-1', roomName: 'ห้องวิทยาศาสตร์' },
					{ name: 'ม.4/2', roomId: null }
				]
			}),
			'staff-1'
		),
		{ classLabel: 'ม.4/1, ม.4/2', roomLabel: 'ห้องวิทยาศาสตร์' }
	);
	assert.equal(
		buildStaffOwnTimetableBlockDisplay(block({ blockKind: 'activity' }), 'staff-1').classLabel,
		'ม.4/1'
	);
});
test('synchronized labels use only the current instructor assigned groups', () => {
	const value = block({
		blockKind: 'activity',
		schedulingMode: 'synchronized',
		groups: [
			group(),
			group({
				homeroomNames: ['ม.6/9'],
				instructors: [{ teacherId: 'other-staff', displayName: 'ครูอื่น' }],
				roomId: 'other-room',
				roomName: 'ห้องของครูอื่น'
			})
		]
	});
	assert.deepEqual(buildStaffOwnTimetableBlockDisplay(value, 'staff-1'), {
		classLabel: 'ม.4/1',
		roomLabel: 'ห้องวิทยาศาสตร์'
	});
	assert.deepEqual(buildStaffOwnTimetableBlockDisplay(value, 'reservation-only'), {
		classLabel: null,
		roomLabel: null
	});
	assert.equal(buildTimetableBlockDisplay(value, 'personal').groupLabel, null);
	assert.equal(buildTimetableBlockDisplay(value, 'personal').roomLabel, null);
});
test('structural blocks omit shared target scope and missing real class names never become aliases', () => {
	assert.deepEqual(
		buildStaffOwnTimetableBlockDisplay(
			block({
				blockKind: 'structural',
				homerooms: [{ name: 'ม.6/9', roomName: 'ห้องของครูอื่น' }]
			}),
			'staff-1'
		),
		{ classLabel: null, roomLabel: null }
	);
	assert.deepEqual(
		buildStaffOwnTimetableBlockDisplay(
			block({ groups: [group({ homeroomNames: [' ', ''], roomId: null, roomName: null })] }),
			'staff-1'
		),
		{ classLabel: null, roomLabel: 'ไม่ระบุห้อง' }
	);
});
