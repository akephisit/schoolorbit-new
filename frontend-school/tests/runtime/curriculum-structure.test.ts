import assert from 'node:assert/strict';
import test from 'node:test';

import {
	buildCurriculumDocument,
	buildProgramComparison
} from '../../src/lib/academic/curriculum-structure.ts';
import type { CurriculumStructureWorkspace } from '../../src/lib/api/academic-core';

const now = '2026-08-29T00:00:00Z';

function workspace(): CurriculumStructureWorkspace {
	return {
		curriculumVersion: {
			id: 'version-1',
			curriculumId: 'curriculum-1',
			versionName: 'ฉบับ 2570',
			startAcademicYearId: 'year-1',
			status: 'draft',
			rowVersion: 1,
			migrated: false,
			createdAt: now,
			updatedAt: now
		},
		termSlots: [
			{
				id: 'term-1',
				curriculumVersionId: 'version-1',
				sequence: 1,
				termType: 'regular',
				typeOccurrence: 1,
				name: 'ภาคเรียนที่ 1',
				rowVersion: 1
			},
			{
				id: 'term-2',
				curriculumVersionId: 'version-1',
				sequence: 2,
				termType: 'regular',
				typeOccurrence: 2,
				name: 'ภาคเรียนที่ 2',
				rowVersion: 1
			},
			{
				id: 'summer',
				curriculumVersionId: 'version-1',
				sequence: 3,
				termType: 'summer',
				typeOccurrence: 1,
				name: 'ภาคฤดูร้อน',
				rowVersion: 1
			}
		],
		programs: [
			{
				id: 'program-a',
				curriculumVersionId: 'version-1',
				code: 'GENERAL',
				nameTh: 'แผนทั่วไป',
				isDefault: true,
				status: 'draft',
				rowVersion: 1,
				createdAt: now,
				updatedAt: now
			},
			{
				id: 'program-b',
				curriculumVersionId: 'version-1',
				code: 'SCI-MATH',
				nameTh: 'วิทย์–คณิต',
				isDefault: false,
				status: 'draft',
				rowVersion: 1,
				createdAt: now,
				updatedAt: now
			}
		],
		gradeLevels: [
			{
				id: 'grade-1',
				code: 'M1',
				name: 'มัธยมศึกษาปีที่ 1',
				short_name: 'ม.1',
				level_type: 'secondary',
				level_order: 301
			}
		],
		requirements: [
			{
				id: 'req-1',
				studyProgramId: 'program-a',
				gradeLevel: {
					id: 'grade-1',
					code: 'M1',
					name: 'มัธยมศึกษาปีที่ 1',
					short_name: 'ม.1',
					level_type: 'secondary',
					level_order: 301
				},
				termSlotId: 'term-1',
				resourceKind: 'course',
				catalogVersionId: 'subject-1',
				code: 'ค21101',
				name: 'คณิตศาสตร์พื้นฐาน',
				section: 'basic_course',
				requirementKind: 'required',
				metrics: {
					weeklyValue: '3',
					weeklyUnit: 'period',
					credit: '1.50',
					totalHours: '60'
				},
				displayOrder: 1
			},
			{
				id: 'req-2',
				studyProgramId: 'program-a',
				gradeLevel: {
					id: 'grade-1',
					code: 'M1',
					name: 'มัธยมศึกษาปีที่ 1',
					short_name: 'ม.1',
					level_type: 'secondary',
					level_order: 301
				},
				termSlotId: 'term-1',
				resourceKind: 'course',
				catalogVersionId: 'subject-2',
				code: 'ว21201',
				name: 'วิทยาศาสตร์เพิ่มเติม',
				section: 'additional_course',
				requirementKind: 'elective',
				metrics: {
					weeklyValue: '2',
					weeklyUnit: 'period',
					credit: '1.00',
					totalHours: '40'
				},
				displayOrder: 2
			},
			{
				id: 'req-3',
				studyProgramId: 'program-b',
				gradeLevel: {
					id: 'grade-1',
					code: 'M1',
					name: 'มัธยมศึกษาปีที่ 1',
					short_name: 'ม.1',
					level_type: 'secondary',
					level_order: 301
				},
				termSlotId: 'term-2',
				resourceKind: 'course',
				catalogVersionId: 'subject-1',
				code: 'ค21101',
				name: 'คณิตศาสตร์พื้นฐาน',
				section: 'basic_course',
				requirementKind: 'required',
				metrics: {
					weeklyValue: '3',
					weeklyUnit: 'period',
					credit: '1.50',
					totalHours: '60'
				},
				displayOrder: 1
			}
		],
		validation: { blockers: [], warnings: [] },
		rowVersion: 1
	};
}

test('curriculum document keeps dynamic term and section order while summing decimals exactly', () => {
		const document = buildCurriculumDocument(workspace(), 'program-a', 'grade-1');

		assert.deepEqual(document.termPanels.map((panel) => panel.name), [
			'ภาคเรียนที่ 1',
			'ภาคเรียนที่ 2',
			'ภาคฤดูร้อน'
		]);
		assert.deepEqual(document.termPanels[0]?.sections.map((section) => section.label), [
			'รายวิชาพื้นฐาน',
			'รายวิชาเพิ่มเติม',
			'กิจกรรมพัฒนาผู้เรียน'
		]);
		assert.equal(document.termPanels[0]?.totalCredits, '2.50');
		assert.equal(document.termPanels[0]?.totalHours, '100.00');
		assert.equal(document.termPanels[2]?.totalCredits, '0.00');
});

test('curriculum comparison keeps program assignments independent', () => {
		const comparison = buildProgramComparison(workspace(), 'grade-1');
		const mathematics = comparison.rows.find((row) => row.catalogVersionId === 'subject-1');

		assert.deepEqual(comparison.programs.map((program) => program.name), [
			'แผนทั่วไป',
			'วิทย์–คณิต'
		]);
		assert.deepEqual(mathematics?.cells['program-a']?.termNames, ['ภาคเรียนที่ 1']);
		assert.deepEqual(mathematics?.cells['program-b']?.termNames, ['ภาคเรียนที่ 2']);
		assert.equal(mathematics?.isDifferent, true);
});
