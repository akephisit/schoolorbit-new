import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
	buildActualCurriculumRows,
	buildActualSubjectActivityRows,
	buildEffectiveStudyPlanRows,
	filterEffectiveStudyPlanVersions,
	actualSubjectActivityReportForWorksheet,
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

	it('aggregates actual courses and activities by subject code or activity identity', () => {
		const rows = buildActualSubjectActivityRows({
			yearName: 'ปีการศึกษา 2569',
			semesters: [{ id: 'sem-1', term: '1', name: 'ภาคเรียนที่ 1' }],
			classrooms: [
				{ id: 'room-1', name: 'ม.1/1', grade_level_name: 'ม.1' },
				{ id: 'room-2', name: 'ม.1/2', grade_level_name: 'ม.1' }
			],
			courses: [
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
				},
				{
					id: 'course-2',
					classroom_id: 'room-2',
					academic_semester_id: 'sem-1',
					subject_code: 'MA21101',
					subject_name_th: 'คณิตศาสตร์',
					subject_type: 'BASIC',
					subject_credit: 1.5,
					subject_hours: 60,
					instructor_name: 'ครูสาม'
				}
			],
			activities: [
				{
					slot_id: 'slot-1',
					classroom_id: 'room-1',
					semester_id: 'sem-1',
					name: 'ชุมนุม',
					activity_type: 'club',
					periods_per_week: 1,
					scheduling_mode: 'independent'
				},
				{
					slot_id: 'slot-2',
					classroom_id: 'room-2',
					semester_id: 'sem-1',
					name: 'ชุมนุม',
					activity_type: 'club',
					periods_per_week: 1,
					scheduling_mode: 'independent'
				}
			],
			courseInstructorsByCourseId: {
				'course-1': [
					{ classroom_course_id: 'course-1', role: 'primary', instructor_name: 'ครูหนึ่ง' },
					{ classroom_course_id: 'course-1', role: 'secondary', instructor_name: 'ครูสอง' }
				],
				'course-2': [
					{ classroom_course_id: 'course-2', role: 'primary', instructor_name: 'ครูสาม' }
				]
			}
		});

		assert.deepEqual(rows, [
			{
				academicYear: 'ปีการศึกษา 2569',
				term: '1',
				itemKind: 'รายวิชา',
				codeOrActivityType: 'MA21101',
				name: 'คณิตศาสตร์',
				itemType: 'BASIC',
				credits: 1.5,
				hours: 60,
				periodsPerWeek: '',
				classroomCount: 2,
				classrooms: 'ม.1/1, ม.1/2',
				instructors: 'ครูหนึ่ง, ครูสอง, ครูสาม',
				classroomDetails: 'ม.1/1: ครูหนึ่ง (หลัก), ครูสอง (ร่วม)\nม.1/2: ครูสาม (หลัก)'
			},
			{
				academicYear: 'ปีการศึกษา 2569',
				term: '1',
				itemKind: 'กิจกรรม',
				codeOrActivityType: 'club',
				name: 'ชุมนุม',
				itemType: 'club',
				credits: '',
				hours: '',
				periodsPerWeek: 1,
				classroomCount: 2,
				classrooms: 'ม.1/1, ม.1/2',
				instructors: '',
				classroomDetails: 'ม.1/1\nม.1/2'
			}
		]);
	});

	it('formats actual subject activity rows as readable report blocks', () => {
		const report = actualSubjectActivityReportForWorksheet([
			{
				academicYear: 'ปีการศึกษา 2569',
				term: '1',
				itemKind: 'รายวิชา',
				codeOrActivityType: 'MA21101',
				name: 'คณิตศาสตร์',
				itemType: 'BASIC',
				credits: 1.5,
				hours: 60,
				periodsPerWeek: '',
				classroomCount: 2,
				classrooms: 'ม.1/1, ม.1/2',
				instructors: 'ครูหนึ่ง, ครูสอง',
				classroomDetails: 'ม.1/1: ครูหนึ่ง (หลัก)\nม.1/2: ครูสอง (หลัก)'
			},
			{
				academicYear: 'ปีการศึกษา 2569',
				term: '1',
				itemKind: 'กิจกรรม',
				codeOrActivityType: 'club',
				name: 'ชุมนุม',
				itemType: 'club',
				credits: '',
				hours: '',
				periodsPerWeek: 1,
				classroomCount: 2,
				classrooms: 'ม.1/1, ม.1/2',
				instructors: '',
				classroomDetails: 'ม.1/1\nม.1/2'
			}
		]);

		assert.deepEqual(report, [
			['ปีการศึกษา 2569'],
			[],
			['ภาคเรียนที่ 1'],
			['รายวิชา'],
			['รหัส/ชื่อวิชา', 'ประเภท', 'หน่วยกิต', 'ชั่วโมงต่อเทอม', 'จำนวนห้อง', 'ครูผู้สอนทั้งหมด'],
			['MA21101 คณิตศาสตร์', 'BASIC', 1.5, 60, 2, 'ครูหนึ่ง, ครูสอง'],
			['เรียนทั้งหมด', 'ม.1/1, ม.1/2'],
			['ห้องเรียน', 'ครูผู้สอน'],
			['ม.1/1', 'ครูหนึ่ง (หลัก)'],
			['ม.1/2', 'ครูสอง (หลัก)'],
			[],
			['กิจกรรม'],
			['ชื่อกิจกรรม', 'ประเภทกิจกรรม', 'คาบต่อสัปดาห์', 'จำนวนห้อง'],
			['ชุมนุม', 'club', 1, 2],
			['เข้าร่วมทั้งหมด', 'ม.1/1, ม.1/2'],
			['ห้องเรียน'],
			['ม.1/1'],
			['ม.1/2']
		]);
	});

	it('sorts classroom names by grade and room number in actual subject activity rows', () => {
		const rows = buildActualSubjectActivityRows({
			yearName: 'ปีการศึกษา 2569',
			semesters: [{ id: 'sem-1', term: '1', name: 'ภาคเรียนที่ 1' }],
			classrooms: [
				{ id: 'room-m2-1', name: 'ม.2/1', grade_level_name: 'ม.2' },
				{ id: 'room-m1-10', name: 'ม.1/10', grade_level_name: 'ม.1' },
				{ id: 'room-m1-2', name: 'ม.1/2', grade_level_name: 'ม.1' }
			],
			courses: [
				{
					id: 'course-m2-1',
					classroom_id: 'room-m2-1',
					academic_semester_id: 'sem-1',
					subject_code: 'SC21101',
					subject_name_th: 'วิทยาศาสตร์',
					subject_type: 'BASIC',
					subject_credit: 1.5,
					subject_hours: 60,
					instructor_name: 'ครู ม2'
				},
				{
					id: 'course-m1-10',
					classroom_id: 'room-m1-10',
					academic_semester_id: 'sem-1',
					subject_code: 'SC21101',
					subject_name_th: 'วิทยาศาสตร์',
					subject_type: 'BASIC',
					subject_credit: 1.5,
					subject_hours: 60,
					instructor_name: 'ครู ม1-10'
				},
				{
					id: 'course-m1-2',
					classroom_id: 'room-m1-2',
					academic_semester_id: 'sem-1',
					subject_code: 'SC21101',
					subject_name_th: 'วิทยาศาสตร์',
					subject_type: 'BASIC',
					subject_credit: 1.5,
					subject_hours: 60,
					instructor_name: 'ครู ม1-2'
				}
			],
			activities: [],
			courseInstructorsByCourseId: {
				'course-m2-1': [
					{ classroom_course_id: 'course-m2-1', role: 'primary', instructor_name: 'ครู ม2' }
				],
				'course-m1-10': [
					{ classroom_course_id: 'course-m1-10', role: 'primary', instructor_name: 'ครู ม1-10' }
				],
				'course-m1-2': [
					{ classroom_course_id: 'course-m1-2', role: 'primary', instructor_name: 'ครู ม1-2' }
				]
			}
		});

		assert.equal(rows[0].classrooms, 'ม.1/2, ม.1/10, ม.2/1');
		assert.equal(
			rows[0].classroomDetails,
			'ม.1/2: ครู ม1-2 (หลัก)\nม.1/10: ครู ม1-10 (หลัก)\nม.2/1: ครู ม2 (หลัก)'
		);
	});
});
