import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { describe, it } from 'node:test';

import {
	buildTeacherLoadExportRows,
	calculateTeacherLoadColumnWidths,
	TEACHER_LOAD_DETAIL_COLUMN_WIDTH_OPTIONS,
	TEACHER_LOAD_SUMMARY_COLUMN_WIDTH_OPTIONS,
	teacherLoadCategoryForEntry
} from '../../src/lib/utils/timetable-teacher-load-export.ts';

function instructor(overrides = {}) {
	return {
		teacherId: 'teacher-a',
		displayName: 'ครูเอ',
		role: 'primary',
		orderIndex: 0,
		...overrides
	};
}

function entry(overrides = {}) {
	return {
		id: 'block-1',
		academicTermId: 'term-1',
		academicYearId: 'year-1',
		bellScheduleId: 'schedule-1',
		bellSchedulePeriodId: 'period-1',
		blockKind: 'course',
		createdAt: '2026-08-24T00:00:00.000Z',
		dayOfWeek: 'MON',
		endTime: '09:20:00',
		groups: [
			{
				id: 'block-group-1',
				learningGroupId: 'group-1',
				learningOfferingId: 'offering-course-1',
				code: 'M1-1',
				name: 'ม.1/1',
				homeroomIds: ['homeroom-1'],
				instructors: [instructor()],
				roomId: null,
				roomCode: null,
				rowVersion: 1,
				isActive: true,
				syncStatus: null
			}
		],
		homerooms: [],
		teachers: [],
		isActive: true,
		learningOfferingId: 'offering-course-1',
		offeringCode: 'MA21101',
		offeringName: 'คณิตศาสตร์',
		periodName: 'คาบ 1',
		rowVersion: 1,
		schedulingMode: null,
		startTime: '08:30:00',
		title: null,
		updatedAt: '2026-08-24T00:00:00.000Z',
		...overrides
	};
}

const math = { id: 'math', name: 'คณิตศาสตร์', displayOrder: 1 };
const science = { id: 'science', name: 'วิทยาศาสตร์', displayOrder: 2 };
function exportRows(blocks, metadata = {}) {
	return buildTeacherLoadExportRows({
		blocks,
		staff: [],
		offeringSubjectGroups: [],
		homerooms: [],
		rooms: [],
		bellPeriods: [],
		...metadata
	});
}

const projectFile = (filePath) => new URL(`../../${filePath}`, import.meta.url);

describe('timetable teacher load export helpers', () => {
	it('classifies canonical block kinds and activity scheduling modes', () => {
		assert.equal(teacherLoadCategoryForEntry(entry({ blockKind: 'course' })), 'course');
		assert.equal(
			teacherLoadCategoryForEntry(entry({ blockKind: 'activity', schedulingMode: 'independent' })),
			'independentActivity'
		);
		assert.equal(
			teacherLoadCategoryForEntry(entry({ blockKind: 'activity', schedulingMode: 'synchronized' })),
			'synchronizedActivity'
		);
		assert.equal(teacherLoadCategoryForEntry(entry({ blockKind: 'structural' })), 'specialPeriod');
	});

	it('counts each exact canonical instructor once per block', () => {
		const rows = exportRows([
			entry({
				groups: [
					{
						...entry().groups[0],
						instructors: [
							instructor(),
							instructor({ teacherId: 'teacher-b', displayName: 'ครูบี', role: 'secondary' })
						]
					}
				]
			})
		]);

		assert.equal(rows.summaryRows.length, 2);
		assert.deepEqual(
			Object.fromEntries(rows.summaryRows.map((row) => [row.teacherId, row.totalPeriods])),
			{
				'teacher-a': 1,
				'teacher-b': 1
			}
		);
	});

	it('keeps one synchronized block as one period while preserving group labels', () => {
		const rows = exportRows([
			entry({
				blockKind: 'activity',
				schedulingMode: 'synchronized',
				groups: [
					{ ...entry().groups[0], id: 'bg-1', name: 'ม.1/1' },
					{ ...entry().groups[0], id: 'bg-2', name: 'ม.1/2' }
				]
			})
		]);

		assert.equal(rows.summaryRows[0].synchronizedActivityPeriods, 1);
		assert.equal(rows.detailRows.length, 1);
		assert.equal(rows.detailRows[0].homeroomName, 'ม.1/1, ม.1/2');
	});

	it('classifies actual teacher and subject affiliations with roles, including multiple affiliations', () => {
		const rows = exportRows(
			[
				entry({
					groups: [
						{
							...entry().groups[0],
							instructors: [
								instructor(),
								instructor({ teacherId: 'teacher-b', role: 'secondary' }),
								instructor({ teacherId: 'teacher-c' })
							]
						}
					]
				})
			],
			{
				staff: [
					{ id: 'teacher-a', subjectGroups: [science, math, math] },
					{ id: 'teacher-b', subjectGroups: [math] },
					{ id: 'teacher-c', subjectGroups: [science] }
				],
				offeringSubjectGroups: [{ learningOfferingId: 'offering-course-1', subjectGroup: math }]
			}
		);
		const byId = new Map(rows.summaryRows.map((row) => [row.teacherId, row]));
		assert.equal(byId.get('teacher-a').homeGroupPrimaryCoursePeriods, 1);
		assert.equal(byId.get('teacher-a').teacherSubjectGroupName, 'คณิตศาสตร์, วิทยาศาสตร์');
		assert.equal(byId.get('teacher-b').homeGroupSecondaryCoursePeriods, 1);
		assert.equal(byId.get('teacher-c').sharedPrimaryCoursePeriods, 1);
		assert.ok(rows.detailRows.every((row) => row.subjectGroupName === 'คณิตศาสตร์'));
		assert.equal(
			rows.summaryRows.reduce((n, row) => n + row.totalPeriods, 0),
			rows.detailRows.length
		);
	});

	it('keeps missing metadata separate from confirmed teaching outside the teacher group', () => {
		const rows = exportRows([entry()]);
		assert.equal(rows.summaryRows[0].unclassifiedCoursePeriods, 1);
		assert.equal(rows.summaryRows[0].sharedPrimaryCoursePeriods, 0);
		assert.equal(rows.summaryRows[0].totalPeriods, 1);
	});

	it('includes exact activity targets and special-period teachers, excluding retired targets and blocks', () => {
		const target = { teacherId: 'teacher-a', displayName: 'ครูเอ', isActive: true };
		const special = entry({
			id: 'special',
			blockKind: 'structural',
			groups: [],
			teachers: [target, { ...target, teacherId: 'retired', isActive: false }],
			title: 'ประชุม'
		});
		const sync = entry({
			id: 'sync',
			blockKind: 'activity',
			schedulingMode: 'synchronized',
			teachers: [target]
		});
		const rows = exportRows([
			special,
			sync,
			entry({ isActive: false }),
			entry({ id: 'inactive-group', groups: [{ ...entry().groups[0], isActive: false }] })
		]);
		assert.equal(rows.summaryRows.length, 1);
		assert.equal(rows.summaryRows[0].specialPeriods, 1);
		assert.equal(rows.summaryRows[0].synchronizedActivityPeriods, 1);
		assert.equal(rows.summaryRows[0].totalPeriods, 2);
		assert.equal(rows.detailRows.find((row) => row.category === 'specialPeriod').title, 'ประชุม');
	});

	it('preserves distinct canonical blocks even when the activity and times coincide', () => {
		const activity = entry({ blockKind: 'activity', schedulingMode: 'synchronized' });
		const rows = exportRows([activity, { ...activity, id: 'another' }]);
		assert.equal(rows.summaryRows[0].totalPeriods, 2);
		assert.equal(rows.detailRows.length, 2);
	});

	it('exports each teacher’s own classes and physical rooms and sorts by bell-period order', () => {
		const block = entry({
			groups: [
				{ ...entry().groups[0], homeroomIds: ['room-a'], roomId: 'physical-a' },
				{
					...entry().groups[0],
					id: 'bg-b',
					homeroomIds: ['room-b'],
					roomId: 'physical-b',
					instructors: [instructor({ teacherId: 'teacher-b' })]
				}
			]
		});
		const rows = exportRows(
			[block, { ...block, id: 'second', bellSchedulePeriodId: 'period-10', periodName: 'คาบ 10' }],
			{
				homerooms: [
					{ id: 'room-a', name: 'ม.1/1' },
					{ id: 'room-b', name: 'ม.1/2' }
				],
				rooms: [
					{ id: 'physical-a', name: 'ห้องคณิตศาสตร์' },
					{ id: 'physical-b', name: 'ห้องวิทยาศาสตร์' }
				],
				bellPeriods: [
					{ id: 'period-1', orderIndex: 1 },
					{ id: 'period-10', orderIndex: 10 }
				]
			}
		);
		assert.equal(rows.detailRows[0].periodName, 'คาบ 1');
		const a = rows.detailRows.find((row) => row.teacherId === 'teacher-a');
		const b = rows.detailRows.find((row) => row.teacherId === 'teacher-b');
		assert.equal(a.homeroomName, 'ม.1/1');
		assert.equal(a.roomName, 'ห้องคณิตศาสตร์');
		assert.equal(b.homeroomName, 'ม.1/2');
		assert.equal(b.roomName, 'ห้องวิทยาศาสตร์');
	});
	it('calculates capped Excel widths for both worksheets', () => {
		const rows = exportRows([
			entry({ offeringName: 'ชื่อรายวิชาที่ยาวมากเพื่อทดสอบการจำกัดความกว้างของคอลัมน์' })
		]);
		const summaryWidths = calculateTeacherLoadColumnWidths(
			rows.summarySheetRows,
			TEACHER_LOAD_SUMMARY_COLUMN_WIDTH_OPTIONS
		);
		const detailWidths = calculateTeacherLoadColumnWidths(
			rows.detailSheetRows,
			TEACHER_LOAD_DETAIL_COLUMN_WIDTH_OPTIONS
		);
		assert.equal(summaryWidths.length, 12);
		assert.equal(detailWidths.length, 11);
		assert.ok(summaryWidths[1] <= 24);
		assert.ok(detailWidths[10] <= 42);
	});

	it('exports canonical blocks with exceljs and TH Sarabun New', () => {
		const workbookModule = readFileSync(
			projectFile('src/lib/utils/timetable-teacher-load-workbook.ts'),
			'utf8'
		);
		assert.match(workbookModule, /TimetableBlock/);
		assert.match(workbookModule, /import\('exceljs'\)/);
		assert.match(workbookModule, /new ExcelJS\.Workbook\(\)/);
		assert.match(workbookModule, /workbook\.xlsx\.writeBuffer\(\)/);
		assert.match(workbookModule, /TH Sarabun New/);
	});

	it('canonical generated contracts contain nested block targets and instructors', () => {
		const frontendApi = readFileSync(projectFile('src/lib/api/timetable.ts'), 'utf8');
		const generated = readFileSync(projectFile('src/lib/api/generated/school-api.ts'), 'utf8');
		assert.match(frontendApi, /export type TimetableBlock = Schemas\['TimetableBlock'\]/);
		assert.match(generated, /TimetableBlockInstructor:[\s\S]*teacherId: string;/);
		assert.match(generated, /TimetableBlockGroup:[\s\S]*instructors:/);
	});
});
