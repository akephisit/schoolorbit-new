import assert from 'node:assert/strict';
import test from 'node:test';

import {
	buildMyExamInvigilationSummary,
	buildStaffExamInvigilatorRenderRows,
	buildStaffExamRoundSummary,
	buildStaffExamScheduleRenderRows,
	filterStaffExamScheduleRound,
	flattenStaffExamScheduleRound,
	formatStaffExamDate,
	formatStaffExamTime,
	groupStaffInvigilatorRowsByDay,
	groupStaffScheduleRowsByDay
} from '../../src/lib/utils/staff-exam-schedule-view.ts';

const staffA = { staffId: 'staff-a', displayName: 'ครู ก' };
const staffB = { staffId: 'staff-b', displayName: 'ครู ข' };

function session({
	sessionId,
	startsAt,
	endsAt,
	subjectCode,
	subjectName,
	gradeLevelYear,
	classroomId,
	classroomName,
	assignmentId,
	roomId,
	roomName
}) {
	return {
		sessionId,
		startsAt,
		endsAt,
		durationMinutes: 60,
		subjectId: `subject-${sessionId}`,
		subjectCode,
		subjectName,
		assessmentCategoryName: 'กลางภาค',
		gradeLevelId: `grade-${gradeLevelYear}`,
		gradeLevelName: `ม.${gradeLevelYear}`,
		gradeLevelType: 'secondary',
		gradeLevelYear,
		classroomId,
		classroomName,
		dayRoomAssignmentId: assignmentId,
		roomId,
		roomName,
		buildingName: 'อาคาร 3'
	};
}

function assignment({
	assignmentId,
	classroomId,
	classroomName,
	roomId,
	roomName,
	startsAt,
	endsAt,
	sessionMinutes,
	invigilators
}) {
	return {
		assignmentId,
		classroomId,
		classroomName,
		roomId,
		roomName,
		buildingName: 'อาคาร 3',
		sessionMinutes,
		earliestStartsAt: startsAt,
		latestEndsAt: endsAt,
		invigilators
	};
}

const round = {
	roundId: 'round-1',
	roundName: 'กลางภาคเรียนที่ 1/2569',
	academicSemesterId: 'semester-1',
	publishedAt: '2026-07-25T08:00:00Z',
	days: [
		{
			examDayId: 'day-1',
			label: 'วันแรก',
			examDate: '2026-08-03',
			sessions: [
				session({
					sessionId: 'session-m1-1',
					startsAt: '08:30:00',
					endsAt: '09:30:00',
					subjectCode: 'ค21101',
					subjectName: 'คณิตศาสตร์',
					gradeLevelYear: 1,
					classroomId: 'class-m1-1',
					classroomName: 'ม.1/1',
					assignmentId: 'assignment-m1-1',
					roomId: 'room-313',
					roomName: '313'
				}),
				session({
					sessionId: 'session-m1-2',
					startsAt: '08:30:00',
					endsAt: '09:30:00',
					subjectCode: 'ว21101',
					subjectName: 'วิทยาศาสตร์',
					gradeLevelYear: 1,
					classroomId: 'class-m1-2',
					classroomName: 'ม.1/2',
					assignmentId: 'assignment-m1-2',
					roomId: 'room-314',
					roomName: '314'
				}),
				{
					...session({
						sessionId: 'session-m4-1',
						startsAt: '10:00:00',
						endsAt: '11:30:00',
						subjectCode: 'ค31101',
						subjectName: 'คณิตศาสตร์เพิ่มเติม',
						gradeLevelYear: 4,
						classroomId: 'class-m4-1',
						classroomName: 'ม.4/1',
						assignmentId: 'assignment-m4-1',
						roomId: 'room-401',
						roomName: '401'
					}),
					durationMinutes: 90
				}
			],
			roomAssignments: [
				assignment({
					assignmentId: 'assignment-m1-1',
					classroomId: 'class-m1-1',
					classroomName: 'ม.1/1',
					roomId: 'room-313',
					roomName: '313',
					startsAt: '08:30:00',
					endsAt: '09:30:00',
					sessionMinutes: 60,
					invigilators: [staffA]
				}),
				assignment({
					assignmentId: 'assignment-m1-2',
					classroomId: 'class-m1-2',
					classroomName: 'ม.1/2',
					roomId: 'room-314',
					roomName: '314',
					startsAt: '08:30:00',
					endsAt: '09:30:00',
					sessionMinutes: 60,
					invigilators: []
				}),
				assignment({
					assignmentId: 'assignment-m4-1',
					classroomId: 'class-m4-1',
					classroomName: 'ม.4/1',
					roomId: 'room-401',
					roomName: '401',
					startsAt: '10:00:00',
					endsAt: '11:30:00',
					sessionMinutes: 90,
					invigilators: [staffB]
				})
			]
		},
		{
			examDayId: 'day-2',
			label: 'วันที่สอง',
			examDate: '2026-08-04',
			sessions: [
				session({
					sessionId: 'session-m1-1-day-2',
					startsAt: '09:00:00',
					endsAt: '10:00:00',
					subjectCode: 'อ21101',
					subjectName: 'ภาษาอังกฤษ',
					gradeLevelYear: 1,
					classroomId: 'class-m1-1',
					classroomName: 'ม.1/1',
					assignmentId: 'assignment-m1-1-day-2',
					roomId: 'room-313',
					roomName: '313'
				})
			],
			roomAssignments: [
				assignment({
					assignmentId: 'assignment-m1-1-day-2',
					classroomId: 'class-m1-1',
					classroomName: 'ม.1/1',
					roomId: 'room-313',
					roomName: '313',
					startsAt: '09:00:00',
					endsAt: '10:00:00',
					sessionMinutes: 60,
					invigilators: [staffA]
				})
			]
		}
	]
};

test('filters lower and upper secondary by grade year', () => {
	const lower = filterStaffExamScheduleRound(round, {
		dayId: 'all',
		level: 'lower_secondary',
		classroomId: 'all',
		query: ''
	});
	assert.deepEqual(
		lower.sessions.map((record) => record.gradeLevelYear),
		[1, 1, 1]
	);

	const upper = filterStaffExamScheduleRound(round, {
		dayId: 'all',
		level: 'upper_secondary',
		classroomId: 'all',
		query: ''
	});
	assert.deepEqual(
		upper.sessions.map((record) => record.gradeLevelYear),
		[4]
	);
});

test('composes day classroom and invigilator search filters', () => {
	const filtered = filterStaffExamScheduleRound(round, {
		dayId: 'day-1',
		level: 'lower_secondary',
		classroomId: 'class-m1-1',
		query: 'ครู ก'
	});

	assert.deepEqual(
		filtered.sessions.map((record) => record.sessionId),
		['session-m1-1']
	);
	assert.deepEqual(
		filtered.assignments.map((record) => record.assignmentId),
		['assignment-m1-1']
	);
});

test('subject search keeps the linked invigilator assignment visible', () => {
	const filtered = filterStaffExamScheduleRound(round, {
		dayId: 'all',
		level: 'all',
		classroomId: 'all',
		query: 'ภาษาอังกฤษ'
	});

	assert.deepEqual(
		filtered.assignments.map((record) => record.assignmentId),
		['assignment-m1-1-day-2']
	);
});

test('never merges day or time spans across group boundaries', () => {
	const rows = buildStaffExamScheduleRenderRows(flattenStaffExamScheduleRound(round).sessions);

	assert.deepEqual(
		rows.map((row) => row.dayRowSpan),
		[3, 0, 0, 1]
	);
	assert.deepEqual(
		rows.map((row) => row.timeRowSpan),
		[2, 0, 1, 1]
	);
	assert.deepEqual(
		rows.map((row) => row.dayGroupIndex),
		[0, 0, 0, 1]
	);
	assert.deepEqual(
		groupStaffScheduleRowsByDay(rows).map((group) => group.rows.length),
		[3, 1]
	);
});

test('recomputes invigilator day spans after filtering', () => {
	const filtered = filterStaffExamScheduleRound(round, {
		dayId: 'day-1',
		level: 'all',
		classroomId: 'all',
		query: ''
	});
	const rows = buildStaffExamInvigilatorRenderRows(filtered.assignments, 'staff-a');

	assert.deepEqual(
		rows.map((row) => row.dayRowSpan),
		[3, 0, 0]
	);
	assert.deepEqual(
		rows.map((row) => row.isCurrentUser),
		[true, false, false]
	);
	assert.deepEqual(
		groupStaffInvigilatorRowsByDay(rows).map((group) => group.rows.length),
		[3]
	);
});

test('summarizes only the current staff assignments', () => {
	const summary = buildMyExamInvigilationSummary(
		flattenStaffExamScheduleRound(round).assignments,
		'staff-a',
		new Date('2026-08-03T07:00:00')
	);

	assert.equal(summary.assignedDayCount, 2);
	assert.equal(summary.assignmentCount, 2);
	assert.equal(summary.totalMinutes, 120);
	assert.equal(
		summary.items.every((item) => item.invigilators.some((staff) => staff.staffId === 'staff-a')),
		true
	);
	assert.deepEqual(
		summary.items.map((item) => item.status),
		['upcoming', 'upcoming']
	);
});

test('builds stable selected-round summary counts', () => {
	const summary = buildStaffExamRoundSummary(round, 'staff-a', new Date('2026-08-03T07:00:00'));

	assert.equal(summary.examDayCount, 2);
	assert.equal(summary.examRoomCount, 3);
	assert.equal(summary.invigilatorCount, 2);
	assert.equal(summary.nextPersonalAssignment?.assignmentId, 'assignment-m1-1');
});

test('keeps invalid display dates and times readable', () => {
	assert.equal(formatStaffExamDate('not-a-date'), 'not-a-date');
	assert.equal(formatStaffExamTime('not-a-time'), 'not-a-time');
	assert.equal(formatStaffExamTime('08:30:00'), '08:30');
});
