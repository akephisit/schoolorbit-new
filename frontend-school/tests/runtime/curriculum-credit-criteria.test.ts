import assert from 'node:assert/strict';
import test from 'node:test';
import {
	assessBasicCredits,
	preserveNewerCreditCriteria,
	applyCreditCriteriaPreset,
	creditCriteriaPresets,
	validateCreditCriteria
} from '../../src/lib/academic-core/curriculum-credit-criteria.ts';
import { curriculumTotals } from '../../src/lib/academic/curriculum-structure.ts';
import type {
	CurriculumCreditCriterion,
	CurriculumStructureRequirement,
	GradeLevelOption
} from '../../src/lib/api/academic-core';
const grade = (year: number): GradeLevelOption => ({
	id: `grade-${year}`,
	code: `M${year}`,
	level_type: 'secondary',
	level_order: 300 + year,
	name: 'ชื่อชั้นที่แก้ไขได้',
	short_name: null
});
const rule: CurriculumCreditCriterion = {
	scope: 'annual',
	comparison: 'equal',
	targetCredits: '22.00',
	enabled: true,
	gradeLevelIds: ['grade-1']
};
const row = (
	creditCategory: 'basic' | 'additional' | null,
	credit: string | null,
	resourceKind: 'course' | 'activity' = 'course'
): CurriculumStructureRequirement => ({
	id: `${creditCategory}-${credit}`,
	studyProgramId: 'program',
	gradeLevel: grade(1),
	termSlotId: 'term',
	resourceKind,
	catalogId: 'catalog',
	catalogVersionId: 'version',
	versionNo: 1,
	code: 'COURSE',
	name: 'รายการ',
	section: resourceKind === 'activity' ? 'student_development' : 'basic_course',
	creditCategory,
	requirementKind: 'required',
	displayOrder: 1,
	hasNewerCurrentVersion: false,
	metrics: {
		credit,
		weeklyUnit: resourceKind === 'activity' ? 'hour' : 'period',
		weeklyValue: '1',
		totalHours: '20'
	}
});
test('credit categories use pinned classification independently of section and activities always contribute zero', () => {
	const totals = curriculumTotals([
		row('basic', '0.10'),
		row('additional', '0.20'),
		row(null, '999', 'activity')
	]);
	assert.deepEqual(totals.basicCredits, { value: '0.10', partial: false });
	assert.deepEqual(totals.additionalCredits, { value: '0.20', partial: false });
	assert.deepEqual(totals.credits, { value: '0.30', partial: false });
	const unknown = curriculumTotals([row('basic', '2'), row(null, '3')]);
	assert.deepEqual(unknown.basicCredits, { value: '2.00', partial: true });
	assert.deepEqual(unknown.additionalCredits, { value: '0.00', partial: true });
	assert.deepEqual(unknown.credits, { value: '5.00', partial: false });
	assert.deepEqual(curriculumTotals([]).basicCredits, { value: '0.00', partial: false });
	assert.equal(curriculumTotals([row('basic', null)]).basicCredits.partial, true);
});
test('equal and minimum assessments distinguish exact deficits/excess and never round through Number', () => {
	assert.equal(
		assessBasicCredits({ value: '22.00', partial: false }, rule, ['grade-1'], 'annual', 'grade-1'),
		null
	);
	assert.match(
		assessBasicCredits({ value: '21.90', partial: false }, rule, ['grade-1'], 'annual', 'grade-1')!
			.message,
		/ขาด 0.10/
	);
	assert.match(
		assessBasicCredits({ value: '22.01', partial: false }, rule, ['grade-1'], 'annual', 'grade-1')!
			.message,
		/เกิน 0.01/
	);
	assert.equal(
		assessBasicCredits(
			{ value: '99999999999999999.99', partial: false },
			{ ...rule, comparison: 'minimum' },
			['grade-1'],
			'annual',
			'grade-1'
		),
		null
	);
	assert.equal(
		assessBasicCredits(
			{ value: '0.00', partial: false },
			{ ...rule, targetCredits: '0' },
			['grade-1'],
			'annual',
			'grade-1'
		),
		null
	);
});
test('partial metrics or changed coverage are unknown, while disabled/unset/other scope never warns', () => {
	for (const value of ['—', '22.00'])
		assert.equal(
			assessBasicCredits({ value, partial: true }, rule, ['grade-1'], 'annual', 'grade-1')?.status,
			'unknown'
		);
	assert.equal(
		assessBasicCredits(
			{ value: '22.00', partial: false },
			rule,
			['grade-1', 'grade-2'],
			'annual',
			'grade-2'
		)?.status,
		'unknown'
	);
	assert.equal(
		assessBasicCredits(
			{ value: '0.00', partial: false },
			{ ...rule, enabled: false },
			['grade-1'],
			'annual',
			'grade-1'
		),
		null
	);
	assert.equal(
		assessBasicCredits(
			{ value: '0.00', partial: false },
			undefined,
			['grade-1'],
			'annual',
			'grade-1'
		),
		null
	);
	assert.equal(
		assessBasicCredits({ value: '0.00', partial: false }, rule, ['grade-1'], 'level'),
		null
	);
});
test('suggested templates use authoritative grade metadata and preserve the unrelated scope without applying automatically', () => {
	assert.equal(
		creditCriteriaPresets([grade(1), grade(2)], ['grade-1', 'grade-2'])[0].criterion.targetCredits,
		'22.00'
	);
	assert.equal(
		creditCriteriaPresets([grade(4), grade(5), grade(6)], ['grade-4', 'grade-5', 'grade-6'])[0]
			.criterion.targetCredits,
		'41.00'
	);
	assert.deepEqual(creditCriteriaPresets([grade(4), grade(5)], ['grade-4', 'grade-5']), []);
	assert.deepEqual(
		creditCriteriaPresets(
			[{ ...grade(1), level_type: 'primary', name: 'มัธยมศึกษาปีที่ 1' }],
			['grade-1']
		),
		[]
	);
	const other = { ...rule, scope: 'level' as const };
	const original = [other];
	const applied = applyCreditCriteriaPreset(original, rule);
	assert.equal(applied[0], other);
	assert.equal(original.length, 1);
	assert.deepEqual(applied[1], rule);
});
test('criteria validation matches provider bounds, zero, canonical syntax, two unique scopes and grade uniqueness', () => {
	for (const targetCredits of ['0', '0.00', '9999.99', '22.5'])
		assert.equal(validateCreditCriteria([{ ...rule, targetCredits }]), '');
	for (const targetCredits of ['', '01.5', '-1', '10000', '1.001', 'NaN'])
		assert.notEqual(validateCreditCriteria([{ ...rule, targetCredits }]), '');
	assert.notEqual(validateCreditCriteria([rule, rule]), '');
	assert.notEqual(validateCreditCriteria([{ ...rule, gradeLevelIds: ['same', 'same'] }]), '');
});

test('independent criteria never regress under older same-draft structure/slot snapshots, with history and draft isolation', () => {
	const incoming = {
		id: 'level',
		draftId: 'draft',
		creditCriteria: [rule],
		creditCriteriaRowVersion: 1,
		other: 'incoming structure metadata'
	};
	const current = {
		...incoming,
		creditCriteria: [{ ...rule, targetCredits: '23' }],
		creditCriteriaRowVersion: 2,
		other: 'old structure metadata'
	};
	const merged = preserveNewerCreditCriteria(current, incoming);
	assert.equal(merged.other, incoming.other);
	assert.equal(merged.creditCriteriaRowVersion, 2);
	assert.equal(merged.creditCriteria, current.creditCriteria);
	assert.deepEqual(
		preserveNewerCreditCriteria({ ...current, creditCriteria: [] }, incoming).creditCriteria,
		[]
	);
	assert.equal(
		preserveNewerCreditCriteria(current, { ...incoming, creditCriteriaRowVersion: 3 })
			.creditCriteriaRowVersion,
		3
	);
	assert.equal(
		preserveNewerCreditCriteria(current, { ...incoming, draftId: 'other-draft' })
			.creditCriteriaRowVersion,
		1
	);
	assert.equal(
		preserveNewerCreditCriteria({ ...current, draftId: null }, { ...incoming, draftId: null })
			.creditCriteriaRowVersion,
		1
	);
	assert.equal(incoming.creditCriteriaRowVersion, 1);
});
