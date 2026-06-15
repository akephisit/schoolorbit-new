import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
	buildActualCurriculumRows,
	buildEffectiveStudyPlanRows,
	filterEffectiveStudyPlanVersions,
	summarizeActualCurriculum
} from '../../src/lib/utils/curriculum-export.ts';

const academicYears = [
	{ id: 'year-2568', year: 2568, name: 'ปีการศึกษา 2568' },
	{ id: 'year-2569', year: 2569, name: 'ปีการศึกษา 2569' },
	{ id: 'year-2570', year: 2570, name: 'ปีการศึกษา 2570' }
];

describe('curriculum export helpers', () => {
	it('filters study plan versions whose effective range covers the selected year', () => {
		const versions = [
			{
				id: 'current',
				study_plan_id: 'plan-general',
				version_name: 'v2569',
				start_academic_year_id: 'year-2569',
				end_academic_year_id: null,
				is_active: true
			},
			{
				id: 'ended-before',
				study_plan_id: 'plan-old',
				version_name: 'v2568',
				start_academic_year_id: 'year-2568',
				end_academic_year_id: 'year-2568',
				is_active: true
			},
			{
				id: 'future',
				study_plan_id: 'plan-future',
				version_name: 'v2570',
				start_academic_year_id: 'year-2570',
				end_academic_year_id: null,
				is_active: true
			}
		];

		assert.deepEqual(
			filterEffectiveStudyPlanVersions(versions, academicYears, 'year-2569').map((v) => v.id),
			['current']
		);
	});

	it('builds planned curriculum rows for subjects and activities', () => {
		const rows = buildEffectiveStudyPlanRows({
			version: {
				id: 'version-1',
				study_plan_id: 'plan-general',
				study_plan_name_th: 'แผนทั่วไป',
				version_name: 'v2569',
				start_academic_year_id: 'year-2569',
				end_academic_year_id: null
			},
			academicYears,
			gradeLevels: [{ id: 'm1', short_name: 'ม.1', name: 'มัธยมศึกษาปีที่ 1' }],
			subjects: [
				{
					id: 'sps-1',
					grade_level_id: 'm1',
					term: '1',
					subject_code: 'TH21101',
					subject_name_th: 'ภาษาไทย',
					subject_type: 'BASIC',
					subject_credit: 1.5,
					subject_hours: 60
				}
			],
			activities: [
				{
					id: 'activity-1',
					grade_level_id: 'm1',
					term: '2',
					catalog_name: 'แนะแนว',
					catalog_activity_type: 'guidance',
					catalog_periods_per_week: 1,
					catalog_scheduling_mode: 'shared'
				}
			]
		});

		assert.deepEqual(
			rows.map((row) => ({
				itemKind: row.itemKind,
				term: row.term,
				name: row.name
			})),
			[
				{ itemKind: 'รายวิชา', term: '1', name: 'ภาษาไทย' },
				{ itemKind: 'กิจกรรม', term: '2', name: 'แนะแนว' }
			]
		);
	});

	it('builds actual curriculum rows and classroom summaries', () => {
		const courses = [
			{
				id: 'course-1',
				classroom_id: 'room-1',
				academic_semester_id: 'sem-1',
				subject_code: 'MA21101',
				subject_name_th: 'คณิตศาสตร์',
				subject_type: 'BASIC',
				subject_credit: 1.5,
				subject_hours: 60,
				instructor_name: 'ครูหนึ่ง'
			}
		];
		const activities = [
			{
				slot_id: 'slot-1',
				classroom_id: 'room-1',
				semester_id: 'sem-1',
				name: 'ชุมนุม',
				activity_type: 'club',
				periods_per_week: 1,
				scheduling_mode: 'independent'
			}
		];
		const rows = buildActualCurriculumRows({
			yearName: 'ปีการศึกษา 2569',
			semesters: [{ id: 'sem-1', term: '1', name: 'ภาคเรียนที่ 1' }],
			classrooms: [{ id: 'room-1', name: 'ม.1/1', grade_level_name: 'ม.1' }],
			courses,
			activities
		});

		assert.deepEqual(
			rows.map((row) => ({ itemKind: row.itemKind, classroom: row.classroom, name: row.name })),
			[
				{ itemKind: 'รายวิชา', classroom: 'ม.1/1', name: 'คณิตศาสตร์' },
				{ itemKind: 'กิจกรรม', classroom: 'ม.1/1', name: 'ชุมนุม' }
			]
		);

		assert.deepEqual(summarizeActualCurriculum(rows), [
			{
				academicYear: 'ปีการศึกษา 2569',
				term: '1',
				classroom: 'ม.1/1',
				gradeLevel: 'ม.1',
				courseCount: 1,
				activityCount: 1
			}
		]);
	});
});
