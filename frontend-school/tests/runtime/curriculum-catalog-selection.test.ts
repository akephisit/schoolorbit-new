import assert from 'node:assert/strict';
import test from 'node:test';
import {
	groupStagedRequirements,
	appendSelectedCatalogRequirements,
	hasExistingCourseRequirement,
	selectableCatalogOption,
	type CurriculumCatalogOption,
	type StagedCurriculumRequirement
} from '../../src/lib/academic-core/curriculum-catalog-selection.ts';
import type { CurriculumStructureRequirement } from '../../src/lib/api/academic-core.ts';

const context = { studyProgramId: 'program-a', gradeLevelId: 'grade-1', termSlotId: 'term-1' };
const current: CurriculumCatalogOption = {
	id: 'subject-v2',
	catalogId: 'subject',
	resourceKind: 'course',
	code: 'ค21101',
	name: 'คณิตศาสตร์',
	gradeLevelIds: ['grade-1', 'grade-2'],
	section: 'basic_course',
	creditCategory: 'basic',
	versionNo: 2,
	effectiveFrom: '2026-05-01',
	selectableForNewRequirement: true
};
const old: CurriculumCatalogOption = {
	...current,
	id: 'subject-v1',
	versionNo: 1,
	selectableForNewRequirement: false
};
const pinned: CurriculumStructureRequirement = {
	id: 'requirement-1',
	...context,
	gradeLevel: {
		id: 'grade-1',
		code: 'M1',
		name: 'ม.1',
		short_name: 'ม.1',
		level_order: 1,
		level_type: 'secondary'
	},
	catalogVersionId: old.id,
	catalogId: old.catalogId,
	versionNo: old.versionNo,
	hasNewerCurrentVersion: true,
	resourceKind: 'course',
	code: old.code,
	name: old.name,
	section: 'basic_course',
	creditCategory: 'basic',
	subjectGroupId: 'math',
	subjectGroupOrder: 1,
	requirementKind: 'required',
	displayOrder: 1,
	metrics: { weeklyUnit: 'period', weeklyValue: '3', credit: '1.50', totalHours: '60' }
};
const existing: StagedCurriculumRequirement = {
	...context,
	catalogVersionId: old.id,
	resourceKind: 'course',
	requirementKind: 'required',
	displayOrder: 1
};

test('add selection rechecks server eligibility and never substitutes an old or future selection', () => {
	const future = {
		...current,
		id: 'subject-v3',
		versionNo: 3,
		effectiveFrom: '2027-05-01',
		selectableForNewRequirement: false
	};
	assert.equal(selectableCatalogOption(old), false);
	assert.equal(selectableCatalogOption(current), true);
	assert.equal(selectableCatalogOption(future), false);
	const added = appendSelectedCatalogRequirements(
		[],
		[old.id, current.id, future.id, 'missing'],
		context,
		[old, current, future],
		[],
		'required'
	);
	assert.deepEqual(
		added.map((item) => item.catalogVersionId),
		[current.id]
	);
});

test('pinned requirement identity blocks a newer revision even when its old option is unavailable', () => {
	assert.deepEqual(
		hasExistingCourseRequirement(current, context, [existing], [current], [pinned]),
		true
	);
	assert.deepEqual(
		appendSelectedCatalogRequirements(
			[existing],
			[current.id],
			context,
			[current],
			[pinned],
			'elective'
		),
		[existing]
	);
});

test('program, grade and term contexts remain independent and additions cannot stage two revisions of one subject', () => {
	for (const changed of [
		{ studyProgramId: 'program-b' },
		{ gradeLevelId: 'grade-2' },
		{ termSlotId: 'term-2' }
	]) {
		const result = appendSelectedCatalogRequirements(
			[existing],
			[current.id],
			{ ...context, ...changed },
			[current],
			[pinned],
			'required'
		);
		assert.equal(result.length, 2);
	}
	const anotherCurrent = { ...current, id: 'another-current-version' };
	const result = appendSelectedCatalogRequirements(
		[],
		[current.id, anotherCurrent.id],
		context,
		[current, anotherCurrent],
		[],
		'required'
	);
	assert.equal(result.length, 1);
	assert.deepEqual(hasExistingCourseRequirement(current, context, result, [current], []), true);
});

test('explicit removal permits replacement while legacy revision duplicates remain untouched', () => {
	assert.equal(
		appendSelectedCatalogRequirements([], [current.id], context, [current], [pinned], 'required')[0]
			?.catalogVersionId,
		current.id
	);
	const legacy = [existing, { ...existing, catalogVersionId: current.id, displayOrder: 2 }];
	const result = appendSelectedCatalogRequirements(
		legacy,
		[current.id],
		context,
		[current],
		[pinned],
		'required'
	);
	assert.deepEqual(result, legacy);
});

test('activity revision selection retains its exact-version policy and never collides with a course', () => {
	const activity = {
		...current,
		id: 'activity-v1',
		resourceKind: 'activity' as const,
		selectableForNewRequirement: true
	};
	const second = { ...activity, id: 'activity-v2', versionNo: 2 };
	assert.equal(selectableCatalogOption(activity), true);
	const result = appendSelectedCatalogRequirements(
		[existing],
		[activity.id, second.id, activity.id],
		context,
		[current, activity, second],
		[pinned],
		'required'
	);
	assert.deepEqual(
		result.map((item) => item.catalogVersionId),
		[old.id, activity.id, second.id]
	);
});

test('concrete grade coverage is rechecked when adding stale picker selections', () => {
	const otherGrade = { ...current, gradeLevelIds: ['grade-2'] };
	assert.equal(selectableCatalogOption(otherGrade, context.gradeLevelId), false);
	assert.deepEqual(
		appendSelectedCatalogRequirements([], [otherGrade.id], context, [otherGrade], [], 'required'),
		[]
	);
});

test('section presentation prefers pinned metadata, preserves every duplicate and retains unknown legacy rows', () => {
	const activity = { ...existing, resourceKind: 'activity' as const, catalogVersionId: 'activity' };
	const unknown = { ...existing, catalogVersionId: 'missing' };
	const groups = groupStagedRequirements(
		[existing, { ...existing }, activity, unknown],
		[
			{ ...old, section: 'additional_course' },
			{ ...current, id: 'activity', resourceKind: 'activity', section: 'student_development' }
		],
		[pinned]
	);
	assert.equal(groups.find((group) => group.id === 'basic_course')!.rows.length, 2);
	assert.equal(groups.find((group) => group.id === 'additional_course')!.rows.length, 0);
	assert.equal(groups.find((group) => group.id === 'student_development')!.rows.length, 1);
	assert.equal(groups.find((group) => group.id === 'unclassified')!.rows.length, 1);
});
test('section grouping retains persisted order values and input identities without rewriting the draft', () => {
	const duplicate = { ...existing, displayOrder: 9 };
	const additional = { ...existing, catalogVersionId: 'additional', displayOrder: 4 };
	const rows = [duplicate, additional, existing];
	const before = structuredClone(rows);
	const groups = groupStagedRequirements(
		rows,
		[{ ...current, id: 'additional', section: 'additional_course' }],
		[pinned]
	);
	assert.deepEqual(groups.find((group) => group.id === 'basic_course')!.rows, [
		existing,
		duplicate
	]);
	assert.deepEqual(groups.find((group) => group.id === 'additional_course')!.rows, [additional]);
	assert.equal(groups.find((group) => group.id === 'basic_course')!.rows[1], duplicate);
	assert.deepEqual(rows, before);
});
