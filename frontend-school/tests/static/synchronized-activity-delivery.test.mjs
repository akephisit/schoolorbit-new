import assert from 'node:assert/strict';
import test from 'node:test';

const deliveryModule =
	await import('../../src/lib/academic/synchronized-activity-delivery.ts').catch(() => ({}));

const synchronizedItem = {
	requirementId: 'club-requirement-a',
	resourceKind: 'activity',
	catalogVersionId: 'club-version',
	code: 'CLUB',
	name: 'ชุมนุม',
	requirementKind: 'required',
	standardPeriodsPerWeek: null,
	weeklyPeriodTarget: null,
	schedulingMode: 'synchronized',
	offeringId: null,
	offeringState: 'missing',
	groupMode: 'missing',
	teacherState: 'missing_primary',
	timetableState: 'unscheduled',
	alignmentStates: ['curriculum_requirement_not_offered'],
	groups: []
};

function room(id, programId, item) {
	return {
		homeroom: { id, name: id, gradeLevel: null, gradeLevelId: null },
		gradeLevel: { id: `${id}-grade`, code: 'M1', name: 'มัธยมศึกษาปีที่ 1' },
		studyProgram: {
			id: programId,
			code: programId,
			name: programId,
			curriculumId: 'curriculum',
			curriculumName: 'หลักสูตรทดสอบ'
		},
		curriculumVersionId: 'curriculum-version',
		expectedCount: 1,
		readyCount: 0,
		items: [item],
		extraOfferings: [],
		blockers: []
	};
}

test('a missing synchronized activity opens one preparation target across every matching program', () => {
	assert.equal(
		typeof deliveryModule.buildSynchronizedActivityPreparationTarget,
		'function',
		'the synchronized activity preparation helper must exist'
	);
	const workspace = {
		academicTermId: 'term',
		academicYearId: 'year',
		timetableVersionId: null,
		timetableVersionStatus: null,
		timetableVersionEffectiveFrom: null,
		homerooms: [
			room('ม.1/1', 'program-a', synchronizedItem),
			room('ม.1/2', 'program-a', { ...synchronizedItem, requirementId: 'club-requirement-b' }),
			room('ม.1/3', 'program-b', { ...synchronizedItem, requirementId: 'club-requirement-c' })
		],
		unlinked: []
	};

	const target = deliveryModule.buildSynchronizedActivityPreparationTarget(
		workspace,
		'club-version'
	);

	assert.deepEqual(target, {
		resourceKind: 'activity',
		catalogVersionId: 'club-version',
		code: 'CLUB',
		name: 'ชุมนุม',
		studyProgramIds: ['program-a', 'program-b'],
		homeroomCount: 3
	});
});

test('independent or already-open activities do not offer the synchronized preparation action', () => {
	assert.equal(
		typeof deliveryModule.buildSynchronizedActivityPreparationTarget,
		'function',
		'the synchronized activity preparation helper must exist'
	);
	assert.equal(
		deliveryModule.buildSynchronizedActivityPreparationTarget(
			{
				homerooms: [
					room('ม.1/1', 'program-a', { ...synchronizedItem, schedulingMode: 'independent' })
				]
			},
			'club-version'
		),
		null
	);
	assert.equal(
		deliveryModule.buildSynchronizedActivityPreparationTarget(
			{
				homerooms: [room('ม.1/1', 'program-a', { ...synchronizedItem, offeringId: 'offering' })]
			},
			'club-version'
		),
		null
	);
});

test('focused curriculum preparation applies only the selected activity and skips unrelated proposals', () => {
	assert.equal(
		typeof deliveryModule.buildFocusedCurriculumPreparationChoices,
		'function',
		'the focused choice helper must exist'
	);
	const targetProposal = {
		proposalId: 'target-proposal',
		resourceKind: 'activity',
		catalogVersionId: 'club-version',
		groupingState: 'proposed',
		conflicts: [],
		defaultGroups: [{ groupKey: 'group-a', name: 'ชุมนุม', homeroomIds: ['room-a'] }]
	};
	const unrelatedProposal = {
		...targetProposal,
		proposalId: 'unrelated-proposal',
		catalogVersionId: 'guidance-version'
	};
	const focus = { resourceKind: 'activity', catalogVersionId: 'club-version' };

	assert.deepEqual(
		deliveryModule.buildFocusedCurriculumPreparationChoices(
			[targetProposal, unrelatedProposal],
			focus
		),
		[
			{
				proposalId: 'target-proposal',
				action: 'apply',
				groups: [{ groupKey: 'group-a', name: 'ชุมนุม', homeroomIds: ['room-a'] }]
			},
			{ proposalId: 'unrelated-proposal', action: 'skip', groups: [] }
		]
	);
	assert.deepEqual(
		deliveryModule.visibleCurriculumPreparationProposals(
			[targetProposal, unrelatedProposal],
			focus
		),
		[targetProposal]
	);
});
