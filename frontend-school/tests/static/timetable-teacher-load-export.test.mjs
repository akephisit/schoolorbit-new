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

function entry(overrides) {
	return {
		id: 'entry-1',
		entry_type: 'COURSE',
		day_of_week: 'MON',
		period_id: 'period-1',
		period_name: 'คาบ 1',
		period_order_index: 1,
		start_time: '08:30:00',
		end_time: '09:20:00',
		classroom_name: 'ม.1/1',
		subject_code: 'MA21101',
		subject_name_th: 'คณิตศาสตร์',
		subject_group_id: 'math-group',
		subject_group_name: 'คณิตศาสตร์',
		subject_group_display_order: 1,
		instructor_ids: ['teacher-a'],
		instructor_names: ['ครูเอ'],
		instructor_roles: ['primary'],
		instructor_subject_group_ids: ['math-group'],
		instructor_subject_group_names: ['คณิตศาสตร์'],
		instructor_subject_group_display_orders: [1],
		...overrides
	};
}

function projectFile(path) {
	return new URL(`../../${path}`, import.meta.url);
}

function workspaceFile(path) {
	return new URL(`../../../${path}`, import.meta.url);
}

describe('timetable teacher load export helpers', () => {
	it('classifies courses and activity scheduling modes', () => {
		assert.equal(teacherLoadCategoryForEntry(entry({ entry_type: 'COURSE' })), 'course');
		assert.equal(
			teacherLoadCategoryForEntry(
				entry({ entry_type: 'ACTIVITY', activity_scheduling_mode: 'independent' })
			),
			'independentActivity'
		);
		assert.equal(
			teacherLoadCategoryForEntry(
				entry({ entry_type: 'ACTIVITY', activity_scheduling_mode: 'synchronized' })
			),
			'synchronizedActivity'
		);
		assert.equal(
			teacherLoadCategoryForEntry(
				entry({ entry_type: 'ACTIVITY', activity_scheduling_mode: null })
			),
			'unspecifiedActivity'
		);
		assert.equal(
			teacherLoadCategoryForEntry(entry({ entry_type: 'ACTIVITY', activity_scheduling_mode: '' })),
			'unspecifiedActivity'
		);
		assert.equal(
			teacherLoadCategoryForEntry(
				entry({ entry_type: 'ACTIVITY', activity_scheduling_mode: 'custom' })
			),
			'unspecifiedActivity'
		);
		assert.equal(teacherLoadCategoryForEntry(entry({ entry_type: 'BREAK' })), null);
		assert.equal(teacherLoadCategoryForEntry(entry({ entry_type: 'HOMEROOM' })), null);
		assert.equal(teacherLoadCategoryForEntry(entry({ entry_type: 'ACADEMIC' })), null);
	});

	it('splits course periods by teacher subject group and teacher role', () => {
		const rows = buildTeacherLoadExportRows([
			entry({
				id: 'course-1',
				instructor_ids: ['teacher-a', 'teacher-b', 'teacher-c', 'teacher-d'],
				instructor_names: ['ครูเอ', 'ครูบี', 'ครูซี', 'ครูดี'],
				instructor_roles: ['primary', 'secondary', 'primary', 'secondary'],
				instructor_subject_group_ids: [
					'math-group',
					'math-group',
					'science-group',
					'science-group'
				],
				instructor_subject_group_names: [
					'คณิตศาสตร์',
					'คณิตศาสตร์',
					'วิทยาศาสตร์และเทคโนโลยี',
					'วิทยาศาสตร์และเทคโนโลยี'
				],
				instructor_subject_group_display_orders: [1, 1, 3, 3]
			}),
			entry({
				id: 'guidance-1',
				entry_type: 'ACTIVITY',
				activity_scheduling_mode: 'independent',
				activity_slot_id: 'guidance-slot',
				title: 'แนะแนว',
				subject_code: undefined,
				subject_name_th: undefined,
				subject_group_id: undefined,
				subject_group_name: undefined,
				subject_group_display_order: undefined,
				instructor_ids: ['teacher-a'],
				instructor_names: ['ครูเอ'],
				instructor_roles: ['primary'],
				instructor_subject_group_ids: ['math-group'],
				instructor_subject_group_names: ['คณิตศาสตร์'],
				instructor_subject_group_display_orders: [1]
			})
		]);

		assert.deepEqual(rows.summaryRows, [
			{
				teacherId: 'teacher-a',
				teacherName: 'ครูเอ',
				teacherSubjectGroupId: 'math-group',
				teacherSubjectGroupName: 'คณิตศาสตร์',
				teacherSubjectGroupDisplayOrder: 1,
				homeGroupPrimaryCoursePeriods: 1,
				homeGroupSecondaryCoursePeriods: 0,
				sharedPrimaryCoursePeriods: 0,
				sharedSecondaryCoursePeriods: 0,
				independentActivityPeriods: 1,
				synchronizedActivityPeriods: 0,
				unspecifiedActivityPeriods: 0,
				totalPeriods: 2
			},
			{
				teacherId: 'teacher-b',
				teacherName: 'ครูบี',
				teacherSubjectGroupId: 'math-group',
				teacherSubjectGroupName: 'คณิตศาสตร์',
				teacherSubjectGroupDisplayOrder: 1,
				homeGroupPrimaryCoursePeriods: 0,
				homeGroupSecondaryCoursePeriods: 1,
				sharedPrimaryCoursePeriods: 0,
				sharedSecondaryCoursePeriods: 0,
				independentActivityPeriods: 0,
				synchronizedActivityPeriods: 0,
				unspecifiedActivityPeriods: 0,
				totalPeriods: 1
			},
			{
				teacherId: 'teacher-c',
				teacherName: 'ครูซี',
				teacherSubjectGroupId: 'science-group',
				teacherSubjectGroupName: 'วิทยาศาสตร์และเทคโนโลยี',
				teacherSubjectGroupDisplayOrder: 3,
				homeGroupPrimaryCoursePeriods: 0,
				homeGroupSecondaryCoursePeriods: 0,
				sharedPrimaryCoursePeriods: 1,
				sharedSecondaryCoursePeriods: 0,
				independentActivityPeriods: 0,
				synchronizedActivityPeriods: 0,
				unspecifiedActivityPeriods: 0,
				totalPeriods: 1
			},
			{
				teacherId: 'teacher-d',
				teacherName: 'ครูดี',
				teacherSubjectGroupId: 'science-group',
				teacherSubjectGroupName: 'วิทยาศาสตร์และเทคโนโลยี',
				teacherSubjectGroupDisplayOrder: 3,
				homeGroupPrimaryCoursePeriods: 0,
				homeGroupSecondaryCoursePeriods: 0,
				sharedPrimaryCoursePeriods: 0,
				sharedSecondaryCoursePeriods: 1,
				independentActivityPeriods: 0,
				synchronizedActivityPeriods: 0,
				unspecifiedActivityPeriods: 0,
				totalPeriods: 1
			}
		]);
		assert.deepEqual(
			rows.summaryGroups.map((group) => ({
				name: group.subjectGroupName,
				rows: group.rows.map((row) => row.teacherName)
			})),
			[
				{ name: 'คณิตศาสตร์', rows: ['ครูเอ', 'ครูบี'] },
				{ name: 'วิทยาศาสตร์และเทคโนโลยี', rows: ['ครูซี', 'ครูดี'] }
			]
		);
		assert.equal(rows.detailRows[0].teacherSubjectGroupName, 'คณิตศาสตร์');
		assert.equal(rows.detailRows[0].subjectGroupName, 'คณิตศาสตร์');
		assert.equal(rows.detailRows[0].categoryLabel, 'วิชาในกลุ่มสาระ (ครูหลัก)');
		assert.equal(rows.detailRows[1].teacherSubjectGroupName, 'คณิตศาสตร์');
		assert.equal(rows.detailRows[1].subjectGroupName, 'คณิตศาสตร์');
		assert.equal(rows.detailRows[1].categoryLabel, 'วิชาในกลุ่มสาระ (ครูรอง)');
		assert.equal(rows.detailRows[2].teacherSubjectGroupName, 'คณิตศาสตร์');
		assert.equal(rows.detailRows[2].subjectGroupName, 'กิจกรรม');
		assert.equal(rows.detailRows[3].teacherSubjectGroupName, 'วิทยาศาสตร์และเทคโนโลยี');
		assert.equal(rows.detailRows[3].subjectGroupName, 'คณิตศาสตร์');
		assert.equal(rows.detailRows[3].categoryLabel, 'วิชานอกกลุ่มสาระ (ครูหลัก)');
		assert.equal(rows.detailRows[4].teacherSubjectGroupName, 'วิทยาศาสตร์และเทคโนโลยี');
		assert.equal(rows.detailRows[4].subjectGroupName, 'คณิตศาสตร์');
		assert.equal(rows.detailRows[4].categoryLabel, 'วิชานอกกลุ่มสาระ (ครูรอง)');
		assert.deepEqual(rows.summarySheetRows[0], [
			'กลุ่มสาระครู',
			'ครูผู้สอน',
			'วิชาในกลุ่มสาระ (ครูหลัก)',
			'วิชาในกลุ่มสาระ (ครูรอง)',
			'วิชานอกกลุ่มสาระ (ครูหลัก)',
			'วิชานอกกลุ่มสาระ (ครูรอง)',
			'กิจกรรม independent (คาบ)',
			'กิจกรรม synchronized (คาบ)',
			'กิจกรรมไม่ระบุประเภท (คาบ)',
			'รวม (คาบ)'
		]);
	});

	it('keeps unspecified activity modes separate from synchronized activities', () => {
		const rows = buildTeacherLoadExportRows([
			entry({
				id: 'manual-activity-1',
				entry_type: 'ACTIVITY',
				activity_scheduling_mode: null,
				title: 'กิจกรรมเพิ่มเอง',
				subject_code: undefined,
				subject_name_th: undefined,
				subject_group_id: undefined,
				subject_group_name: undefined,
				instructor_ids: ['teacher-a'],
				instructor_names: ['ครูเอ']
			}),
			entry({
				id: 'club-1',
				entry_type: 'ACTIVITY',
				activity_scheduling_mode: 'synchronized',
				activity_slot_id: 'club-slot',
				title: 'ชุมนุม',
				subject_code: undefined,
				subject_name_th: undefined,
				subject_group_id: undefined,
				subject_group_name: undefined,
				instructor_ids: ['teacher-a'],
				instructor_names: ['ครูเอ']
			})
		]);

		assert.equal(rows.summaryRows[0].synchronizedActivityPeriods, 1);
		assert.equal(rows.summaryRows[0].unspecifiedActivityPeriods, 1);
		assert.equal(rows.summaryRows[0].totalPeriods, 2);
		assert.deepEqual(
			rows.detailRows.map((row) => row.categoryLabel),
			['กิจกรรม synchronized', 'กิจกรรมไม่ระบุประเภท']
		);
	});

	it('deduplicates synchronized activities across classrooms for the same teacher, slot, day, and period', () => {
		const rows = buildTeacherLoadExportRows([
			entry({
				id: 'scout-m1-1',
				entry_type: 'ACTIVITY',
				activity_scheduling_mode: 'synchronized',
				activity_slot_id: 'scout-slot',
				title: 'ลูกเสือ',
				classroom_name: 'ม.1/1',
				subject_code: undefined,
				subject_name_th: undefined,
				subject_group_id: undefined,
				subject_group_name: undefined,
				instructor_ids: ['teacher-a'],
				instructor_names: ['ครูเอ']
			}),
			entry({
				id: 'scout-m1-2',
				entry_type: 'ACTIVITY',
				activity_scheduling_mode: 'synchronized',
				activity_slot_id: 'scout-slot',
				title: 'ลูกเสือ',
				classroom_name: 'ม.1/2',
				subject_code: undefined,
				subject_name_th: undefined,
				subject_group_id: undefined,
				subject_group_name: undefined,
				instructor_ids: ['teacher-a'],
				instructor_names: ['ครูเอ']
			})
		]);

		assert.equal(rows.summaryRows[0].synchronizedActivityPeriods, 1);
		assert.equal(rows.summaryRows[0].totalPeriods, 1);
		assert.equal(rows.detailRows.length, 1);
		assert.equal(rows.detailRows[0].classroomName, 'ม.1/1, ม.1/2');
	});

	it('calculates compact Excel column widths from sheet content', () => {
		assert.deepEqual(
			calculateTeacherLoadColumnWidths(
				[
					['ครู', 'จำนวน'],
					['ครูเอ', 12]
				],
				{ minWidths: [8, 6], maxWidths: [20, 8], padding: 2 }
			),
			[8, 7]
		);
		assert.deepEqual(
			calculateTeacherLoadColumnWidths([['หัวคอลัมน์ยาวมาก'], ['สั้น']], {
				minWidths: [6],
				maxWidths: [10],
				padding: 2
			}),
			[10]
		);
	});

	it('uses capped auto-fit widths for teacher load workbook sheets', () => {
		const exportRows = buildTeacherLoadExportRows([
			entry({
				id: 'course-1',
				instructor_ids: ['teacher-a'],
				instructor_names: ['ครูเอ'],
				instructor_subject_group_ids: ['math-group'],
				instructor_subject_group_names: ['คณิตศาสตร์'],
				instructor_subject_group_display_orders: [1]
			}),
			entry({
				id: 'manual-activity-1',
				entry_type: 'ACTIVITY',
				activity_scheduling_mode: null,
				title: 'กิจกรรมเพิ่มเองที่มีชื่อค่อนข้างยาวเพื่อทดสอบการจำกัดความกว้าง',
				subject_code: undefined,
				subject_name_th: undefined,
				subject_group_id: undefined,
				subject_group_name: undefined,
				instructor_ids: ['teacher-a'],
				instructor_names: ['ครูเอ']
			})
		]);

		const summaryWidths = calculateTeacherLoadColumnWidths(
			exportRows.summarySheetRows,
			TEACHER_LOAD_SUMMARY_COLUMN_WIDTH_OPTIONS
		);
		const detailWidths = calculateTeacherLoadColumnWidths(
			exportRows.detailSheetRows,
			TEACHER_LOAD_DETAIL_COLUMN_WIDTH_OPTIONS
		);

		assert.equal(summaryWidths.length, 10);
		assert.equal(detailWidths.length, 9);
		assert.ok(summaryWidths[1] <= 24);
		assert.ok(summaryWidths.slice(2, 9).every((width) => width <= 14));
		assert.ok(detailWidths[8] <= 42);
	});

	it('exports the teacher load workbook with exceljs and TH Sarabun New', () => {
		const page = readFileSync(
			projectFile('src/routes/(app)/staff/academic/timetable/+page.svelte'),
			'utf8'
		);
		const exportFunction = page.slice(page.indexOf('async function handleExportTeacherLoadXlsx'));
		const summarySheetFunction = page.slice(
			page.indexOf('function appendTeacherLoadSummarySheet'),
			page.indexOf('function appendTeacherLoadDetailSheet')
		);

		assert.match(exportFunction, /import\('exceljs'\)/);
		assert.match(exportFunction, /new ExcelJS\.Workbook\(\)/);
		assert.match(exportFunction, /workbook\.xlsx\.writeBuffer\(\)/);
		assert.match(summarySheetFunction, /กิจกรรมไม่ระบุประเภท/);
		assert.match(summarySheetFunction, /unspecifiedActivityPeriods/);
		assert.match(summarySheetFunction, /styleTeacherLoadGroupRow\(groupRow, 10\)/);
		assert.match(page, /calculateTeacherLoadColumnWidths/);
		assert.match(page, /TEACHER_LOAD_SUMMARY_COLUMN_WIDTH_OPTIONS/);
		assert.match(page, /TEACHER_LOAD_DETAIL_COLUMN_WIDTH_OPTIONS/);
		assert.match(page, /TH Sarabun New/);
		assert.doesNotMatch(exportFunction, /import\('xlsx'\)/);
		assert.doesNotMatch(exportFunction, /XLSX\.writeFile/);
	});

	it('keeps teacher load subject-group fields aligned across backend and frontend', () => {
		const frontendApi = readFileSync(projectFile('src/lib/api/timetable.ts'), 'utf8');
		const backendModel = readFileSync(
			workspaceFile('backend-school/src/modules/academic/models/timetable.rs'),
			'utf8'
		);
		const backendService = readFileSync(
			workspaceFile('backend-school/src/modules/academic/services/timetable_service.rs'),
			'utf8'
		);

		assert.match(frontendApi, /subject_group_id\?: string \| null/);
		assert.match(frontendApi, /subject_group_name\?: string \| null/);
		assert.match(frontendApi, /instructor_roles\?: string\[\]/);
		assert.match(frontendApi, /instructor_subject_group_ids\?: Array<string \| null> \| null/);
		assert.match(backendModel, /pub subject_group_id: Option<Uuid>/);
		assert.match(backendModel, /pub subject_group_name: Option<String>/);
		assert.match(backendModel, /pub instructor_roles: Option<Vec<String>>/);
		assert.match(backendModel, /pub instructor_subject_group_ids: Option<Vec<Option<Uuid>>>/);
		assert.match(backendService, /s\.group_id AS subject_group_id/);
		assert.match(backendService, /sg\.name_th AS subject_group_name/);
		assert.match(backendService, /AS instructor_roles/);
		assert.match(backendService, /AS instructor_subject_group_ids/);
		assert.match(backendService, /organization_members om/);
	});
});
