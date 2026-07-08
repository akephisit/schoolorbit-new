import assert from 'node:assert/strict';
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
		instructor_ids: ['teacher-a'],
		instructor_names: ['ครูเอ'],
		...overrides
	};
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

	it('counts every co-teacher once for course and independent activity periods', () => {
		const rows = buildTeacherLoadExportRows([
			entry({
				id: 'course-1',
				instructor_ids: ['teacher-a', 'teacher-b'],
				instructor_names: ['ครูเอ', 'ครูบี']
			}),
			entry({
				id: 'guidance-1',
				entry_type: 'ACTIVITY',
				activity_scheduling_mode: 'independent',
				activity_slot_id: 'guidance-slot',
				title: 'แนะแนว',
				subject_code: undefined,
				subject_name_th: undefined,
				instructor_ids: ['teacher-a'],
				instructor_names: ['ครูเอ']
			})
		]);

		assert.deepEqual(rows.summaryRows, [
			{
				teacherId: 'teacher-a',
				teacherName: 'ครูเอ',
				coursePeriods: 1,
				independentActivityPeriods: 1,
				synchronizedActivityPeriods: 0,
				totalPeriods: 2
			},
			{
				teacherId: 'teacher-b',
				teacherName: 'ครูบี',
				coursePeriods: 1,
				independentActivityPeriods: 0,
				synchronizedActivityPeriods: 0,
				totalPeriods: 1
			}
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
				instructor_ids: ['teacher-a'],
				instructor_names: ['ครูเอ']
			})
		]);

		assert.equal(rows.summaryRows[0].synchronizedActivityPeriods, 1);
		assert.equal(rows.summaryRows[0].totalPeriods, 1);
		assert.equal(rows.detailRows.length, 1);
		assert.equal(rows.detailRows[0].classroomName, 'ม.1/1, ม.1/2');
	});
});
