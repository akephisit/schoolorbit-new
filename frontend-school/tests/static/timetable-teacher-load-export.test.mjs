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
		userId: 'teacher-a',
		displayName: 'ครูเอ',
		role: 'primary',
		subjectGroupId: 'math-group',
		subjectGroupName: 'คณิตศาสตร์',
		subjectGroupDisplayOrder: 1,
		...overrides
	};
}

function entry(overrides = {}) {
	return {
		id: 'entry-1',
		academicTermId: 'term-1',
		academicYearId: 'year-1',
		bellScheduleId: 'schedule-1',
		bellSchedulePeriodId: 'period-1',
		createdAt: '2026-08-24T00:00:00.000Z',
		dayOfWeek: 'MON',
		endTime: '09:20:00',
		entryType: 'COURSE',
		homeroomName: 'ม.1/1',
		instructors: [instructor()],
		isActive: true,
		offeringCode: 'MA21101',
		offeringId: 'offering-course-1',
		offeringName: 'คณิตศาสตร์',
		periodName: 'คาบ 1',
		rowVersion: 1,
		startTime: '08:30:00',
		subjectGroupId: 'math-group',
		subjectGroupName: 'คณิตศาสตร์',
		subjectGroupDisplayOrder: 1,
		subjectVersionDisplayLabel: 'คณิตศาสตร์ · v1',
		updatedAt: '2026-08-24T00:00:00.000Z',
		...overrides
	};
}

function projectFile(path) {
	return new URL(`../../${path}`, import.meta.url);
}

function workspaceFile(path) {
	return new URL(`../../../${path}`, import.meta.url);
}

function readTimetableServices() {
	return readFileSync(
		workspaceFile('backend-school/src/modules/academic/services/timetable_service.rs'),
		'utf8'
	);
}

describe('timetable teacher load export helpers', () => {
	it('classifies canonical course and activity scheduling modes', () => {
		assert.equal(teacherLoadCategoryForEntry(entry({ entryType: 'COURSE' })), 'course');
		assert.equal(
			teacherLoadCategoryForEntry(
				entry({ entryType: 'ACTIVITY', activitySchedulingMode: 'independent' })
			),
			'independentActivity'
		);
		assert.equal(
			teacherLoadCategoryForEntry(
				entry({ entryType: 'ACTIVITY', activitySchedulingMode: 'synchronized' })
			),
			'synchronizedActivity'
		);
		assert.equal(
			teacherLoadCategoryForEntry(entry({ entryType: 'ACTIVITY', activitySchedulingMode: null })),
			'unspecifiedActivity'
		);
		assert.equal(teacherLoadCategoryForEntry(entry({ entryType: 'BREAK' })), null);
	});

	it('splits course periods by nested instructor subject group and role', () => {
		const rows = buildTeacherLoadExportRows([
			entry({
				instructors: [
					instructor(),
					instructor({
						userId: 'teacher-b',
						displayName: 'ครูบี',
						role: 'secondary',
						subjectGroupId: 'thai-group',
						subjectGroupName: 'ภาษาไทย',
						subjectGroupDisplayOrder: 2
					})
				]
			})
		]);

		assert.equal(rows.summaryRows.length, 2);
		assert.deepEqual(
			rows.summaryRows.map((row) => [
				row.teacherId,
				row.homeGroupPrimaryCoursePeriods,
				row.sharedSecondaryCoursePeriods,
				row.totalPeriods
			]),
			[
				['teacher-a', 1, 0, 1],
				['teacher-b', 0, 1, 1]
			]
		);
	});

	it('deduplicates synchronized offerings while preserving all homeroom labels', () => {
		const rows = buildTeacherLoadExportRows([
			entry({
				id: 'sync-a',
				entryType: 'ACTIVITY',
				activitySchedulingMode: 'synchronized',
				offeringId: 'activity-offering-1',
				offeringName: 'ลูกเสือ',
				homeroomName: 'ม.1/1',
				subjectGroupId: null,
				subjectGroupName: null,
				subjectGroupDisplayOrder: null
			}),
			entry({
				id: 'sync-b',
				entryType: 'ACTIVITY',
				activitySchedulingMode: 'synchronized',
				offeringId: 'activity-offering-1',
				offeringName: 'ลูกเสือ',
				homeroomName: 'ม.1/2',
				subjectGroupId: null,
				subjectGroupName: null,
				subjectGroupDisplayOrder: null
			})
		]);

		assert.equal(rows.summaryRows[0].synchronizedActivityPeriods, 1);
		assert.equal(rows.summaryRows[0].totalPeriods, 1);
		assert.equal(rows.detailRows.length, 1);
		assert.equal(rows.detailRows[0].homeroomName, 'ม.1/1, ม.1/2');
	});

	it('keeps independent activities as separate teaching periods', () => {
		const rows = buildTeacherLoadExportRows([
			entry({
				id: 'independent-a',
				entryType: 'ACTIVITY',
				activitySchedulingMode: 'independent',
				offeringId: 'activity-offering-1',
				homeroomName: 'ม.1/1'
			}),
			entry({
				id: 'independent-b',
				entryType: 'ACTIVITY',
				activitySchedulingMode: 'independent',
				offeringId: 'activity-offering-1',
				homeroomName: 'ม.1/2'
			})
		]);

		assert.equal(rows.summaryRows[0].independentActivityPeriods, 2);
		assert.equal(rows.detailRows.length, 2);
	});

	it('calculates capped Excel widths for both worksheets', () => {
		const rows = buildTeacherLoadExportRows([
			entry({ offeringName: 'ชื่อชุดการเรียนที่ยาวมากเพื่อทดสอบการจำกัดความกว้างของคอลัมน์' })
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

	it('exports the teacher load workbook with exceljs and TH Sarabun New', () => {
		const page = readFileSync(
			projectFile('src/routes/(app)/staff/academic/timetable/+page.svelte'),
			'utf8'
		);
		const workbookModule = readFileSync(
			projectFile('src/lib/utils/timetable-teacher-load-workbook.ts'),
			'utf8'
		);

		assert.match(page, /import\('\$lib\/utils\/timetable-teacher-load-workbook'\)/);
		assert.match(workbookModule, /import\('exceljs'\)/);
		assert.match(workbookModule, /new ExcelJS\.Workbook\(\)/);
		assert.match(workbookModule, /workbook\.xlsx\.writeBuffer\(\)/);
		assert.match(workbookModule, /calculateTeacherLoadColumnWidths/);
		assert.match(workbookModule, /TEACHER_LOAD_SUMMARY_COLUMN_WIDTH_OPTIONS/);
		assert.match(workbookModule, /TEACHER_LOAD_DETAIL_COLUMN_WIDTH_OPTIONS/);
		assert.match(workbookModule, /TH Sarabun New/);
		assert.doesNotMatch(workbookModule, /import\('xlsx'\)/);
	});

	it('keeps nested subject-group fields aligned across generated and Rust contracts', () => {
		const frontendApi = readFileSync(projectFile('src/lib/api/timetable.ts'), 'utf8');
		const generated = readFileSync(projectFile('src/lib/api/generated/school-api.ts'), 'utf8');
		const backendModel = readFileSync(
			workspaceFile('backend-school/src/modules/academic/models/timetable.rs'),
			'utf8'
		);
		const backendService = readTimetableServices();

		assert.match(frontendApi, /export type TimetableEntry = Schemas\['TimetableEntry'\]/);
		assert.match(generated, /TimetableInstructor:[\s\S]*subjectGroupId\?: string \| null;/);
		assert.match(generated, /TimetableEntry:[\s\S]*subjectGroupName\?: string \| null;/);
		assert.match(backendModel, /pub subject_group_id: Option<Uuid>/);
		assert.match(backendModel, /pub subject_group_name: Option<String>/);
		assert.match(backendModel, /pub subject_group_display_order: Option<i32>/);
		assert.match(backendService, /subject_group\.name_th AS subject_group_name/);
		assert.match(backendService, /organization_members membership/);
	});
});
