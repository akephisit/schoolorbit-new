import assert from 'node:assert/strict';
import test from 'node:test';

import {
	buildCurriculumMatrix,
	curriculumTotals,
	curriculumMetric,
	matrixCellKey,
	buildCurriculumDocument,
	buildCurriculumValidationNoticeViews,
	buildProgramComparison
} from '../../src/lib/academic/curriculum-structure.ts';
import type { CurriculumStructureWorkspace } from '../../src/lib/api/academic-core';

const now = '2026-08-29T00:00:00Z';

function workspace(): CurriculumStructureWorkspace {
	return {
		level: {
			id: 'version-1',
			editionId: 'curriculum-1',
			code: 'J',
			nameTh: 'ระดับมัธยมศึกษาตอนต้น',
			gradeLevelIds: ['grade-1'],
			creditCriteria: [],
			creditCriteriaRowVersion: 1,
			isActive: true,
			editionName: 'ฉบับ 2570',
			revisionYear: 2570,
			status: 'draft',
			rowVersion: 1,
			migrated: false,
			createdAt: now,
			updatedAt: now
		},
		termSlots: [
			{
				id: 'term-1',
				curriculumLevelId: 'version-1',
				sequence: 1,
				termType: 'regular',
				typeOccurrence: 1,
				name: 'ภาคเรียนที่ 1',
				rowVersion: 1
			},
			{
				id: 'term-2',
				curriculumLevelId: 'version-1',
				sequence: 2,
				termType: 'regular',
				typeOccurrence: 2,
				name: 'ภาคเรียนที่ 2',
				rowVersion: 1
			},
			{
				id: 'summer',
				curriculumLevelId: 'version-1',
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
				curriculumLevelId: 'version-1',
				code: 'GENERAL',
				displayOrder: 1,
				nameTh: 'แผนทั่วไป',
				status: 'draft',
				rowVersion: 1,
				createdAt: now,
				updatedAt: now
			},
			{
				id: 'program-b',
				curriculumLevelId: 'version-1',
				code: 'SCI-MATH',
				displayOrder: 2,
				nameTh: 'วิทย์–คณิต',
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
				catalogId: 'subject-stable-1',
				versionNo: 1,
				hasNewerCurrentVersion: true,
				code: 'ค21101',
				name: 'คณิตศาสตร์พื้นฐาน',
				section: 'basic_course',
				creditCategory: 'basic',
				subjectGroupId: 'math',
				subjectGroupOrder: 1,
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
				catalogId: 'subject-stable-2',
				versionNo: 2,
				hasNewerCurrentVersion: false,
				code: 'ว21201',
				name: 'วิทยาศาสตร์เพิ่มเติม',
				section: 'additional_course',
				subjectGroupId: 'science',
				subjectGroupOrder: 2,
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
				catalogId: 'subject-stable-1',
				versionNo: 1,
				hasNewerCurrentVersion: true,
				code: 'ค21101',
				name: 'คณิตศาสตร์พื้นฐาน',
				section: 'basic_course',
				subjectGroupId: 'math',
				subjectGroupOrder: 1,
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

test('curriculum validation notices collapse duplicate API rows into unique render keys', () => {
	const views = buildCurriculumValidationNoticeViews([
		{
			code: 'catalog_total_hours_missing',
			message: 'ค21101 คณิตศาสตร์พื้นฐาน ยังไม่มีชั่วโมงรวมต่อภาคเรียน',
			catalogVersionId: 'subject-1'
		},
		{
			code: 'catalog_total_hours_missing',
			message: 'ค21101 คณิตศาสตร์พื้นฐาน ยังไม่มีชั่วโมงรวมต่อภาคเรียน',
			catalogVersionId: 'subject-1'
		},
		{
			code: 'catalog_weekly_value_missing',
			message: 'ค21101 คณิตศาสตร์พื้นฐาน ยังไม่มีจำนวนคาบหรือชั่วโมงต่อสัปดาห์',
			catalogVersionId: 'subject-1'
		}
	]);

	assert.equal(views.length, 2);
	assert.equal(new Set(views.map((view) => view.key)).size, views.length);
	assert.deepEqual(
		views.map((view) => view.code),
		['catalog_total_hours_missing', 'catalog_weekly_value_missing']
	);
});

function entries(view: ReturnType<typeof buildCurriculumMatrix>) {
	return view.grades.flatMap(({ sections }) =>
		sections.flatMap(({ rows }) => rows.flatMap((row) => Object.values(row.cells).flat()))
	);
}

test('document and comparison share aligned rows, all configured terms including empty summer, and exact totals', () => {
	const source = workspace();
	const document = buildCurriculumDocument(source, 'program-a', 'grade-1');
	assert.deepEqual(
		document.terms.map((term) => term.name),
		['ภาคเรียนที่ 1', 'ภาคเรียนที่ 2', 'ภาคฤดูร้อน']
	);
	assert.deepEqual(
		document.grades[0].sections.map((section) => section.label),
		['รายวิชาพื้นฐาน', 'รายวิชาเพิ่มเติม', 'กิจกรรมพัฒนาผู้เรียน']
	);
	assert.equal(document.summaries[0].selected.credits.value, '2.50');
	assert.equal(document.summaries[0].selected.hours.value, '100.00');
	assert.equal(document.summaries[0].weekly[2].totals.credits.value, '0.00');
	const comparison = buildProgramComparison(source, 'grade-1');
	assert.deepEqual(
		document.grades[0].sections[0].rows.map((row) => row.key),
		comparison.grades[0].sections[0].rows.map((row) => row.key)
	);
	const row = comparison.grades[0].sections[0].rows[0];
	assert.equal(row.cells[matrixCellKey('program-a', 'term-1')][0].metrics.credit, '1.50');
	assert.equal(row.cells[matrixCellKey('program-b', 'term-2')][0].metrics.credit, '1.50');
	assert.equal(row.cells[matrixCellKey('program-b', 'term-1')], undefined);
});

test('stable identity retains different pinned names, versions, warnings and every duplicate in its actual cell', () => {
	const source = workspace();
	const first = source.requirements[0];
	source.requirements[2] = {
		...source.requirements[2],
		versionNo: 2,
		catalogVersionId: 'new-version',
		name: 'ชื่อใหม่',
		hasNewerCurrentVersion: false,
		metrics: { ...first.metrics, credit: '2.00' }
	};
	source.requirements.push({
		...first,
		id: 'duplicate',
		metrics: { ...first.metrics, credit: '0.25' }
	});
	const view = buildProgramComparison(source, 'grade-1');
	assert.equal(entries(view).length, source.requirements.length);
	const cells = view.grades[0].sections[0].rows[0].cells;
	assert.equal(cells[matrixCellKey('program-a', 'term-1')].length, 2);
	assert.equal(cells[matrixCellKey('program-a', 'term-1')][0].hasNewerCurrentVersion, true);
	assert.equal(cells[matrixCellKey('program-b', 'term-2')][0].hasNewerCurrentVersion, false);
	assert.equal(view.summaries[0].selected.credits.value, '2.75');
});

test('unique exact names then Arabic/Thai trailing numbers pair only within the same group and section', () => {
	const source = workspace();
	const first = source.requirements[0];
	const row = (id: string, name: string, termSlotId = 'term-1') => ({
		...first,
		id,
		catalogId: id,
		catalogVersionId: id,
		name,
		termSlotId
	});
	source.requirements = [
		row('s1', 'วิทยาศาสตร์ 1'),
		row('s2', 'วิทยาศาสตร์ ๒', 'term-2'),
		row('design', 'การออกแบบ'),
		row('computing', 'วิทยาการคำนวณ', 'term-2')
	];
	let rows = buildProgramComparison(source, 'grade-1').grades[0].sections[0].rows;
	assert.equal(rows.length, 3);
	assert.equal(
		Object.values(rows.find((item) => item.label === 'วิทยาศาสตร์')!.cells).flat().length,
		2
	);
	source.requirements.push(row('ambiguous', 'วิทยาศาสตร์ 3'));
	rows = buildProgramComparison(source, 'grade-1').grades[0].sections[0].rows;
	assert.equal(rows.length, 5);
	source.requirements = [
		row('one', 'ชื่อเดียวกัน'),
		{ ...row('two', 'ชื่อเดียวกัน', 'term-2'), subjectGroupId: 'another', subjectGroupOrder: 0 }
	];
	assert.equal(buildProgramComparison(source, 'grade-1').grades[0].sections[0].rows.length, 2);
	source.requirements[1].subjectGroupId = first.subjectGroupId;
	assert.equal(buildProgramComparison(source, 'grade-1').grades[0].sections[0].rows.length, 1);
	source.requirements[1].section = 'additional_course';
	assert.equal(
		buildProgramComparison(source, 'grade-1').grades[0].sections.flatMap((section) => section.rows)
			.length,
		2
	);
});

test('all grades, selected terms and whole-level totals have independent scope without changing persisted rows', () => {
	const source = workspace();
	const secondGrade = { ...source.gradeLevels[0], id: 'grade-2', level_order: 302 };
	source.gradeLevels.push(secondGrade);
	source.requirements.push({
		...source.requirements[0],
		id: 'grade-2-row',
		gradeLevel: secondGrade
	});
	const before = structuredClone(source);
	const view = buildCurriculumMatrix(source, 'all', ['term-1']);
	assert.equal(view.grades.length, 2);
	assert.deepEqual(
		view.terms.map((term) => term.id),
		['term-1']
	);
	assert.equal(view.summaries[0].selected.credits.value, '4.00');
	assert.equal(view.summaries[1].selected.credits.value, '0.00');
	assert.equal(view.summaries[1].level.credits.value, '1.50');
	assert.equal(view.summaries[0].weekly.length, 6);
	assert.deepEqual(source, before);
});

test('exact decimal sums exceed Number precision; missing values stay partial and weekly units never mix', () => {
	const first = workspace().requirements[0];
	const rows = [
		{ ...first, metrics: { ...first.metrics, credit: '9007199254740993.01', weeklyValue: '0.10' } },
		{
			...first,
			metrics: { ...first.metrics, credit: '0.09', weeklyValue: '0.20', totalHours: null }
		},
		{
			...first,
			resourceKind: 'activity' as const,
			metrics: { credit: '999', weeklyUnit: 'hour' as const, weeklyValue: '2.50', totalHours: '40' }
		}
	];
	const totals = curriculumTotals(rows);
	assert.deepEqual(totals.credits, { value: '9007199254740993.10', partial: false });
	assert.deepEqual(totals.weeklyPeriods, { value: '0.30', partial: false });
	assert.deepEqual(totals.weeklyHours, { value: '2.50', partial: false });
	assert.deepEqual(totals.hours, { value: '100.00', partial: true });
	assert.deepEqual(
		curriculumTotals([{ ...first, metrics: { ...first.metrics, credit: 'invalid' } }]).credits,
		{ value: '—', partial: true }
	);
	assert.deepEqual(curriculumTotals([]).credits, { value: '0.00', partial: false });
	for (const invalid of [null, '', 'NaN', '-1', '0.001'])
		assert.equal(curriculumMetric(invalid), '—');
});

test('old-status is preserved per requirement directly from server current metadata', () => {
	const source = workspace();
	for (const requirement of source.requirements) requirement.hasNewerCurrentVersion = false;
	assert.ok(
		entries(buildProgramComparison(source, 'grade-1')).every((row) => !row.hasNewerCurrentVersion)
	);
	source.requirements[0].hasNewerCurrentVersion = true;
	assert.equal(
		entries(buildCurriculumDocument(source, 'program-a', 'grade-1')).filter(
			(row) => row.hasNewerCurrentVersion
		).length,
		1
	);
});

test('school weekly hours add teaching periods and activity hours in one concrete term without converting stored values', () => {
	const first = workspace().requirements[0];
	const activity = {
		...first,
		resourceKind: 'activity' as const,
		metrics: { ...first.metrics, weeklyUnit: 'hour' as const, weeklyValue: '3' }
	};
	assert.deepEqual(
		curriculumTotals([{ ...first, metrics: { ...first.metrics, weeklyValue: '32' } }, activity])
			.weeklySchoolHours,
		{ value: '35.00', partial: false }
	);
	assert.equal(first.metrics.weeklyUnit, 'period');
	assert.deepEqual(
		curriculumTotals([{ ...first, metrics: { ...first.metrics, weeklyValue: null } }])
			.weeklySchoolHours,
		{ value: '—', partial: true }
	);
	assert.deepEqual(
		curriculumTotals([
			{ ...first, metrics: { ...first.metrics, weeklyValue: '0.10' } },
			{ ...activity, metrics: { ...activity.metrics, weeklyValue: '0.20' } }
		]).weeklySchoolHours,
		{ value: '0.30', partial: false }
	);
	assert.deepEqual(curriculumTotals([]).weeklySchoolHours, { value: '0.00', partial: false });
});

test('school weekly hours are unavailable across terms, grades or programs without marking the scope incomplete', () => {
	const first = workspace().requirements[0];
	for (const other of [
		{ ...first, termSlotId: 'term-2' },
		{ ...first, gradeLevel: { ...first.gradeLevel, id: 'grade-2' } },
		{ ...first, studyProgramId: 'program-b' }
	]) {
		const totals = curriculumTotals([first, other]);
		assert.deepEqual(totals.weeklySchoolHours, { value: '—', partial: false });
		assert.deepEqual(totals.weeklyPeriods, { value: '6.00', partial: false });
	}
});
