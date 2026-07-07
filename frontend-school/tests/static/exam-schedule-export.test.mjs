import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { buildExamScheduleExportWorkbook } from '../../src/lib/utils/exam-schedule-export.ts';

const baseSession = {
	examScheduleItemId: 'item-1',
	examRoundId: 'round-1',
	examDayId: 'day-1',
	academicSemesterId: 'semester-1',
	assessmentCategoryId: 'category-1',
	assessmentPlanId: 'plan-1',
	classroomCourseId: 'course-1',
	classroomId: 'classroom-1',
	subjectId: 'subject-1',
	gradeLevelId: 'grade-1',
	durationMinutes: 60,
	importedAt: '2026-03-01T00:00:00.000Z',
	examDate: '2026-03-04',
	subjectGroupName: 'คณิตศาสตร์',
	subjectType: 'BASIC',
	roomName: '313',
	buildingName: 'อาคารเรียน',
	invigilators: []
};

const roomAssignments = [
	{
		id: 'room-m1-1',
		examDayId: 'day-1',
		classroomId: 'classroom-m1-1',
		roomId: 'exam-room-1',
		classroomName: 'ม.1/1',
		roomName: '313',
		roomCapacity: 40,
		invigilators: []
	},
	{
		id: 'room-m1-2',
		examDayId: 'day-1',
		classroomId: 'classroom-m1-2',
		roomId: 'exam-room-2',
		classroomName: 'ม.1/2',
		roomName: '314',
		roomCapacity: 40,
		invigilators: []
	},
	{
		id: 'room-m4-1',
		examDayId: 'day-1',
		classroomId: 'classroom-m4-1',
		roomId: 'exam-room-3',
		classroomName: 'ม.4/1',
		roomName: '411',
		roomCapacity: 40,
		invigilators: []
	},
	{
		id: 'room-m4-2',
		examDayId: 'day-1',
		classroomId: 'classroom-m4-2',
		roomId: 'exam-room-4',
		classroomName: 'ม.4/2',
		roomName: '412',
		roomCapacity: 40,
		invigilators: []
	},
	{
		id: 'room-m4-3',
		examDayId: 'day-1',
		classroomId: 'classroom-m4-3',
		roomId: 'exam-room-5',
		classroomName: 'ม.4/3',
		roomName: '413',
		roomCapacity: 40,
		invigilators: []
	}
];

function scheduledSession(overrides) {
	return {
		...baseSession,
		...overrides
	};
}

function exportWorkspace(scheduledSessions) {
	return {
		round: {
			id: 'round-1',
			academicSemesterId: 'semester-1',
			name: 'วัดผลกลางภาคเรียนที่ 2 ปีการศึกษา 2568',
			examKind: 'midterm',
			status: 'draft',
			createdAt: '2026-03-01T00:00:00.000Z',
			updatedAt: '2026-03-01T00:00:00.000Z'
		},
		days: [
			{
				id: 'day-1',
				examRoundId: 'round-1',
				examDate: '2026-03-04',
				label: null,
				startTime: '09:00:00',
				endTime: '15:00:00',
				gradeLevelIds: ['grade-1', 'grade-4'],
				blockedWindows: [],
				roomAssignments
			}
		],
		unscheduledItems: [],
		scheduledSessions,
		readiness: {
			canPublish: true,
			blockers: []
		}
	};
}

describe('exam schedule export helpers', () => {
	it('builds report sheets with full-grade and partial-classroom labels', () => {
		const workbook = buildExamScheduleExportWorkbook(
			exportWorkspace([
				scheduledSession({
					id: 'session-m1-1',
					startsAt: '09:00:00',
					endsAt: '10:00:00',
					durationMinutes: 60,
					classroomName: 'ม.1/1',
					classroomId: 'classroom-m1-1',
					gradeLevelName: 'ม.1',
					gradeLevelYear: 1,
					subjectNameTh: 'คณิตศาสตร์พื้นฐาน',
					subjectCode: 'ค21102'
				}),
				scheduledSession({
					id: 'session-m1-2',
					startsAt: '09:00:00',
					endsAt: '10:00:00',
					durationMinutes: 60,
					classroomName: 'ม.1/2',
					classroomId: 'classroom-m1-2',
					gradeLevelName: 'ม.1',
					gradeLevelYear: 1,
					subjectNameTh: 'คณิตศาสตร์พื้นฐาน',
					subjectCode: 'ค21102'
				}),
				scheduledSession({
					id: 'session-physics',
					startsAt: '09:00:00',
					endsAt: '10:00:00',
					durationMinutes: 60,
					classroomId: 'classroom-m4-1',
					classroomName: 'ม.4/1',
					gradeLevelId: 'grade-4',
					gradeLevelName: 'ม.4',
					gradeLevelYear: 4,
					subjectId: 'subject-physics',
					subjectNameTh: 'ฟิสิกส์',
					subjectCode: 'ว30201'
				}),
				scheduledSession({
					id: 'session-chemistry-m4-2',
					startsAt: '10:00:00',
					endsAt: '11:00:00',
					durationMinutes: 60,
					classroomId: 'classroom-m4-2',
					classroomName: 'ม.4/2',
					gradeLevelId: 'grade-4',
					gradeLevelName: 'ม.4',
					gradeLevelYear: 4,
					subjectId: 'subject-chemistry',
					subjectNameTh: 'เคมี',
					subjectCode: 'ว30221'
				}),
				scheduledSession({
					id: 'session-chemistry-m4-3',
					startsAt: '10:00:00',
					endsAt: '11:00:00',
					durationMinutes: 60,
					classroomId: 'classroom-m4-3',
					classroomName: 'ม.4/3',
					gradeLevelId: 'grade-4',
					gradeLevelName: 'ม.4',
					gradeLevelYear: 4,
					subjectId: 'subject-chemistry',
					subjectNameTh: 'เคมี',
					subjectCode: 'ว30221'
				})
			]),
			null
		);

		assert.deepEqual(
			workbook.reportSheets.map((sheet) => sheet.name),
			['รายงาน', 'ม.ต้น', 'ม.ปลาย']
		);
		assert.equal(workbook.report.rows[0][0], 'ตารางสอบวัดผลกลางภาคเรียนที่ 2 ปีการศึกษา 2568');
		assert.equal(workbook.report.rows[1][0], 'ระดับชั้นมัธยมศึกษา (ม.1 - ม.4)');
		assert.deepEqual(workbook.report.rows[3], [
			'วันเดือนปี',
			'เวลา',
			'เวลาสอบ',
			'วิชา',
			'รหัสวิชา',
			'ชั้น'
		]);
		assert.equal(workbook.report.rows[4][1], '09.00-10.00 น.');
		assert.equal(workbook.report.rows[4][2], '1 ชม.');
		assert.equal(workbook.report.rows[4][5], 'ม.1');
		assert.equal(workbook.report.rows[5][3], 'ฟิสิกส์');
		assert.equal(workbook.report.rows[5][5], 'ม.4/1');
		assert.equal(workbook.report.rows[6][3], 'เคมี');
		assert.equal(workbook.report.rows[6][5], 'ม.4/2-3');
		assert.equal(
			workbook.lowerSecondaryReport?.rows.some((row) => row.includes('ฟิสิกส์')),
			false
		);
		assert.equal(
			workbook.upperSecondaryReport?.rows.some((row) => row.includes('ฟิสิกส์')),
			true
		);
		assert.equal(workbook.report['!cols']?.length, 6);
		assert.deepEqual(workbook.report['!merges'], [
			{ s: { r: 0, c: 0 }, e: { r: 0, c: 5 } },
			{ s: { r: 1, c: 0 }, e: { r: 1, c: 5 } },
			{ s: { r: 4, c: 0 }, e: { r: 6, c: 0 } },
			{ s: { r: 4, c: 1 }, e: { r: 5, c: 1 } },
			{ s: { r: 4, c: 2 }, e: { r: 5, c: 2 } }
		]);
		assert.equal(workbook.schedule.rows[0].ประเภทวิชา, 'พื้นฐาน');
		assert.ok((workbook.schedule['!cols']?.length ?? 0) > 6);
	});
});
