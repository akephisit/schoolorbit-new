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
		assert.equal(teacherLoadCategoryForEntry(entry({ blockKind: 'structural' })), null);
	});

	it('counts each exact canonical instructor once per block', () => {
		const rows = buildTeacherLoadExportRows([
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
		const rows = buildTeacherLoadExportRows([
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

	it('calculates capped Excel widths for both worksheets', () => {
		const rows = buildTeacherLoadExportRows([
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
		assert.equal(summaryWidths.length, 10);
		assert.equal(detailWidths.length, 9);
		assert.ok(summaryWidths[1] <= 24);
		assert.ok(detailWidths[8] <= 42);
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
