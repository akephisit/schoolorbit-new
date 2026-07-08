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

const invigilatorWorkspace = {
	roundId: 'round-1',
	assignments: [
		{
			assignmentId: 'assignment-m1-1',
			examDayId: 'day-1',
			classroomId: 'classroom-m1-1',
			classroomName: 'ม.1/1',
			roomId: 'exam-room-1',
			roomName: '313',
			sessionMinutes: 180,
			invigilators: [
				{ staffId: 'staff-a', displayName: 'ครู A' },
				{ staffId: 'staff-b', displayName: 'ครู B' },
				{ staffId: 'staff-d', displayName: 'ครู D' }
			]
		},
		{
			assignmentId: 'assignment-m1-2',
			examDayId: 'day-1',
			classroomId: 'classroom-m1-2',
			classroomName: 'ม.1/2',
			roomId: 'exam-room-2',
			roomName: '314',
			sessionMinutes: 180,
			invigilators: [{ staffId: 'staff-c', displayName: 'ครู C' }]
		}
	],
	staffWorkloads: [
		{
			staffId: 'staff-a',
			staffName: 'ครู A',
			totalMinutes: 180,
			assignedDayCount: 1,
			assignmentCount: 1,
			days: [{ examDayId: 'day-1', minutes: 180, assignmentCount: 1 }]
		},
		{
			staffId: 'staff-b',
			staffName: 'ครู B',
			totalMinutes: 60,
			assignedDayCount: 1,
			assignmentCount: 1,
			days: [{ examDayId: 'day-1', minutes: 60, assignmentCount: 1 }]
		}
	]
};

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
			[
				'ตารางสอบรวม',
				'ตารางสอบ ม.ต้น',
				'ตารางสอบ ม.ปลาย',
				'ตารางสอบแยกห้อง ม.ต้น',
				'ตารางสอบแยกห้อง ม.ปลาย',
				'กรรมการคุมสอบ',
				'รับส่งข้อสอบ'
			]
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
		assert.equal(workbook.report.rows[4][0], 'วันพุธที่ 4 มีนาคม 2569');
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
		assert.deepEqual(workbook.lowerSecondaryClassroomReport?.rows[3], [
			'ห้องเรียน',
			'วันเดือนปี',
			'เวลา',
			'เวลาสอบ',
			'วิชา',
			'รหัสวิชา',
			'ห้องสอบ'
		]);
		assert.equal(workbook.lowerSecondaryClassroomReport?.rows[4][0], 'ม.1/1');
		assert.equal(workbook.lowerSecondaryClassroomReport?.rows[4][1], 'วันพุธที่ 4 มีนาคม 2569');
		assert.equal(workbook.lowerSecondaryClassroomReport?.rows[4][4], 'คณิตศาสตร์พื้นฐาน');
		assert.equal(workbook.lowerSecondaryClassroomReport?.rows[5][0], 'ม.1/2');
		assert.equal(workbook.upperSecondaryClassroomReport?.rows[4][0], 'ม.4/1');
		assert.equal(workbook.upperSecondaryClassroomReport?.rows[4][4], 'ฟิสิกส์');
		assert.equal(workbook.upperSecondaryClassroomReport?.rows[5][0], 'ม.4/2');
		assert.equal(workbook.upperSecondaryClassroomReport?.rows[5][4], 'เคมี');
		assert.equal(workbook.upperSecondaryClassroomReport?.rows[6][0], 'ม.4/3');
		assert.equal(workbook.report['!printTitlesRow'], '1:4');
		assert.equal(workbook.lowerSecondaryClassroomReport?.['!printTitlesRow'], '1:4');
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

	it('builds invigilator summary sheet with two aligned invigilator columns', () => {
		const workbook = buildExamScheduleExportWorkbook(exportWorkspace([]), invigilatorWorkspace);

		assert.deepEqual(workbook.invigilatorSummary.rows[3], [
			'วันสอบ',
			'ห้องเรียน',
			'ห้องสอบ',
			'กรรมการคุมสอบ',
			''
		]);
		assert.equal(workbook.invigilatorSummary.rows[4][0], 'วันพุธที่ 4 มีนาคม 2569');
		assert.equal(workbook.invigilatorSummary.rows[4][1], 'ม.1/1');
		assert.equal(workbook.invigilatorSummary.rows[4][2], '313');
		assert.equal(workbook.invigilatorSummary.rows[4][3], 'ครู A');
		assert.equal(workbook.invigilatorSummary.rows[4][4], 'ครู B');
		assert.equal(workbook.invigilatorSummary.rows[5][0], 'วันพุธที่ 4 มีนาคม 2569');
		assert.equal(workbook.invigilatorSummary.rows[5][1], 'ม.1/1');
		assert.equal(workbook.invigilatorSummary.rows[5][2], '313');
		assert.equal(workbook.invigilatorSummary.rows[5][3], 'ครู D');
		assert.equal(workbook.invigilatorSummary.rows[5][4], '');
		assert.equal(workbook.invigilatorSummary.rows[6][0], 'วันพุธที่ 4 มีนาคม 2569');
		assert.equal(workbook.invigilatorSummary.rows[6][1], 'ม.1/2');
		assert.equal(workbook.invigilatorSummary.rows[6][2], '314');
		assert.equal(workbook.invigilatorSummary.rows[6][3], 'ครู C');
		assert.equal(workbook.invigilatorSummary.rows[6][4], '');
		assert.equal(workbook.invigilatorSummary['!printTitlesRow'], '1:4');
		assert.deepEqual(workbook.invigilatorSummary['!merges'], [
			{ s: { r: 0, c: 0 }, e: { r: 0, c: 4 } },
			{ s: { r: 1, c: 0 }, e: { r: 1, c: 4 } },
			{ s: { r: 3, c: 3 }, e: { r: 3, c: 4 } },
			{ s: { r: 4, c: 0 }, e: { r: 6, c: 0 } }
		]);
		assert.equal(workbook.invigilatorSummary.rows[3].includes('จำนวนกรรมการ'), false);
		assert.deepEqual(
			workbook.workloads.rows.map((row) => row.ชื่อกรรมการ),
			['ครู B', 'ครู A']
		);
		assert.deepEqual(Object.keys(workbook.workloads.rows[0]), [
			'ชื่อกรรมการ',
			'ชั่วโมงรวม',
			'จำนวนวัน',
			'จำนวนห้อง'
		]);
	});

	it('builds exam paper transfer sheet for invigilator signatures by subject and classroom', () => {
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
				})
			]),
			invigilatorWorkspace
		);

		assert.equal(workbook.paperTransferReport.name, 'รับส่งข้อสอบ');
		assert.deepEqual(workbook.paperTransferReport.rows[3], [
			'วันสอบ',
			'เวลา',
			'รหัสวิชา',
			'วิชา',
			'ชั้น/ห้อง',
			'ห้องสอบ',
			'กรรมการคุมสอบ',
			'ลงชื่อรับข้อสอบ',
			'เวลารับ',
			'ลงชื่อส่งข้อสอบ',
			'เวลาส่ง',
			'หมายเหตุ'
		]);
		assert.deepEqual(workbook.paperTransferReport.rows[4], [
			'วันพุธที่ 4 มีนาคม 2569',
			'09.00-10.00 น.',
			'ค21102',
			'คณิตศาสตร์พื้นฐาน',
			'ม.1/1',
			'อาคารเรียน / 313',
			'ครู A',
			'',
			'',
			'',
			'',
			''
		]);
		assert.equal(workbook.paperTransferReport.rows[5][6], 'ครู B');
		assert.equal(workbook.paperTransferReport.rows[6][6], 'ครู D');
		assert.equal(workbook.paperTransferReport['!printTitlesRow'], '1:4');
		assert.equal(workbook.paperTransferReport['!cols']?.length, 12);
		assert.deepEqual(workbook.paperTransferReport['!merges'], [
			{ s: { r: 0, c: 0 }, e: { r: 0, c: 11 } },
			{ s: { r: 1, c: 0 }, e: { r: 1, c: 11 } },
			{ s: { r: 4, c: 0 }, e: { r: 6, c: 0 } },
			{ s: { r: 4, c: 1 }, e: { r: 6, c: 1 } },
			{ s: { r: 4, c: 2 }, e: { r: 6, c: 2 } },
			{ s: { r: 4, c: 3 }, e: { r: 6, c: 3 } },
			{ s: { r: 4, c: 4 }, e: { r: 6, c: 4 } },
			{ s: { r: 4, c: 5 }, e: { r: 6, c: 5 } },
			{ s: { r: 4, c: 7 }, e: { r: 6, c: 7 } },
			{ s: { r: 4, c: 8 }, e: { r: 6, c: 8 } },
			{ s: { r: 4, c: 9 }, e: { r: 6, c: 9 } },
			{ s: { r: 4, c: 10 }, e: { r: 6, c: 10 } },
			{ s: { r: 4, c: 11 }, e: { r: 6, c: 11 } }
		]);
	});

	it('merges paper transfer subject names only when the subject code also matches', () => {
		const workbook = buildExamScheduleExportWorkbook(
			exportWorkspace([
				scheduledSession({
					id: 'session-english-code-1',
					startsAt: '09:00:00',
					endsAt: '10:00:00',
					durationMinutes: 60,
					classroomName: 'ม.1/1',
					classroomId: 'classroom-m1-1',
					gradeLevelName: 'ม.1',
					gradeLevelYear: 1,
					subjectNameTh: 'ภาษาอังกฤษ',
					subjectCode: 'อ21102'
				}),
				scheduledSession({
					id: 'session-english-code-2',
					startsAt: '09:00:00',
					endsAt: '10:00:00',
					durationMinutes: 60,
					classroomName: 'ม.1/2',
					classroomId: 'classroom-m1-2',
					gradeLevelName: 'ม.1',
					gradeLevelYear: 1,
					subjectNameTh: 'ภาษาอังกฤษ',
					subjectCode: 'อ21202'
				})
			]),
			invigilatorWorkspace
		);

		const subjectMerges = workbook.paperTransferReport['!merges']?.filter(
			(merge) => merge.s.c === 3
		);
		assert.deepEqual(subjectMerges, [{ s: { r: 4, c: 3 }, e: { r: 6, c: 3 } }]);
		assert.equal(workbook.paperTransferReport.rows[7][2], 'อ21202');
		assert.equal(workbook.paperTransferReport.rows[7][3], 'ภาษาอังกฤษ');
	});
});
