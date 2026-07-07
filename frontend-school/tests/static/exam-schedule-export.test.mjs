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
				gradeLevelIds: ['grade-1', 'grade-2'],
				blockedWindows: [],
				roomAssignments: []
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
	it('builds a print-style report sheet grouped by day and exam time', () => {
		const workbook = buildExamScheduleExportWorkbook(
			exportWorkspace([
				scheduledSession({
					id: 'session-m1',
					startsAt: '09:00:00',
					endsAt: '10:00:00',
					durationMinutes: 60,
					classroomName: 'ม.1/1',
					gradeLevelName: 'ม.1',
					gradeLevelYear: 1,
					subjectNameTh: 'คณิตศาสตร์พื้นฐาน',
					subjectCode: 'ค21102'
				}),
				scheduledSession({
					id: 'session-m2',
					startsAt: '09:00:00',
					endsAt: '10:00:00',
					durationMinutes: 60,
					classroomId: 'classroom-2',
					classroomName: 'ม.2/1',
					gradeLevelId: 'grade-2',
					gradeLevelName: 'ม.2',
					gradeLevelYear: 2,
					subjectNameTh: 'คณิตศาสตร์พื้นฐาน',
					subjectCode: 'ค22102'
				}),
				scheduledSession({
					id: 'session-thai',
					startsAt: '10:00:00',
					endsAt: '11:00:00',
					durationMinutes: 60,
					subjectId: 'subject-2',
					subjectNameTh: 'ภาษาไทยพื้นฐาน',
					subjectCode: 'ท21102'
				})
			]),
			null
		);

		assert.equal(workbook.report.rows[0][0], 'ตารางสอบวัดผลกลางภาคเรียนที่ 2 ปีการศึกษา 2568');
		assert.equal(workbook.report.rows[1][0], 'ระดับชั้นมัธยมศึกษาตอนต้น (ม.1 - ม.2)');
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
		assert.equal(workbook.report.rows[5][5], 'ม.2');
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
