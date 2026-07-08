import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { describe, it } from 'node:test';

import {
	buildTeacherLoadExportRows,
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
		assert.equal(teacherLoadCategoryForEntry(entry({ entry_type: 'BREAK' })), null);
		assert.equal(teacherLoadCategoryForEntry(entry({ entry_type: 'HOMEROOM' })), null);
		assert.equal(teacherLoadCategoryForEntry(entry({ entry_type: 'ACADEMIC' })), null);
	});

	it('splits course periods by teacher subject group and keeps activity counts separate', () => {
		const rows = buildTeacherLoadExportRows([
			entry({
				id: 'course-1',
				instructor_ids: ['teacher-a', 'teacher-b'],
				instructor_names: ['ครูเอ', 'ครูบี'],
				instructor_subject_group_ids: ['math-group', 'science-group'],
				instructor_subject_group_names: ['คณิตศาสตร์', 'วิทยาศาสตร์และเทคโนโลยี'],
				instructor_subject_group_display_orders: [1, 3]
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
				homeGroupCoursePeriods: 1,
				sharedCoursePeriods: 0,
				independentActivityPeriods: 1,
				synchronizedActivityPeriods: 0,
				totalPeriods: 2
			},
			{
				teacherId: 'teacher-b',
				teacherName: 'ครูบี',
				teacherSubjectGroupId: 'science-group',
				teacherSubjectGroupName: 'วิทยาศาสตร์และเทคโนโลยี',
				teacherSubjectGroupDisplayOrder: 3,
				homeGroupCoursePeriods: 0,
				sharedCoursePeriods: 1,
				independentActivityPeriods: 0,
				synchronizedActivityPeriods: 0,
				totalPeriods: 1
			}
		]);
		assert.deepEqual(
			rows.summaryGroups.map((group) => ({
				name: group.subjectGroupName,
				rows: group.rows.map((row) => row.teacherName)
			})),
			[
				{ name: 'คณิตศาสตร์', rows: ['ครูเอ'] },
				{ name: 'วิทยาศาสตร์และเทคโนโลยี', rows: ['ครูบี'] }
			]
		);
		assert.equal(rows.detailRows[0].teacherSubjectGroupName, 'คณิตศาสตร์');
		assert.equal(rows.detailRows[0].subjectGroupName, 'คณิตศาสตร์');
		assert.equal(rows.detailRows[0].categoryLabel, 'วิชาในกลุ่มสาระ');
		assert.equal(rows.detailRows[1].teacherSubjectGroupName, 'คณิตศาสตร์');
		assert.equal(rows.detailRows[1].subjectGroupName, 'กิจกรรม');
		assert.equal(rows.detailRows[2].teacherSubjectGroupName, 'วิทยาศาสตร์และเทคโนโลยี');
		assert.equal(rows.detailRows[2].subjectGroupName, 'คณิตศาสตร์');
		assert.equal(rows.detailRows[2].categoryLabel, 'วิชานอกกลุ่มสาระ/สอนร่วม');
		assert.deepEqual(rows.summarySheetRows[0], [
			'กลุ่มสาระครู',
			'ครูผู้สอน',
			'วิชาในกลุ่มสาระ (คาบ)',
			'วิชานอกกลุ่มสาระ/สอนร่วม (คาบ)',
			'กิจกรรม independent (คาบ)',
			'กิจกรรม synchronized (คาบ)',
			'รวม (คาบ)'
		]);
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

	it('exports the teacher load workbook with exceljs and TH Sarabun New', () => {
		const page = readFileSync(
			projectFile('src/routes/(app)/staff/academic/timetable/+page.svelte'),
			'utf8'
		);
		const exportFunction = page.slice(page.indexOf('async function handleExportTeacherLoadXlsx'));

		assert.match(exportFunction, /import\('exceljs'\)/);
		assert.match(exportFunction, /new ExcelJS\.Workbook\(\)/);
		assert.match(exportFunction, /workbook\.xlsx\.writeBuffer\(\)/);
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
		assert.match(frontendApi, /instructor_subject_group_ids\?: Array<string \| null> \| null/);
		assert.match(backendModel, /pub subject_group_id: Option<Uuid>/);
		assert.match(backendModel, /pub subject_group_name: Option<String>/);
		assert.match(backendModel, /pub instructor_subject_group_ids: Option<Vec<Option<Uuid>>>/);
		assert.match(backendService, /s\.group_id AS subject_group_id/);
		assert.match(backendService, /sg\.name_th AS subject_group_name/);
		assert.match(backendService, /AS instructor_subject_group_ids/);
		assert.match(backendService, /organization_members om/);
	});
});
