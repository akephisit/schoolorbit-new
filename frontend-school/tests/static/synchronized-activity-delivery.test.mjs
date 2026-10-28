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
			editionId: 'curriculum',
			levelName: 'หลักสูตรทดสอบ'
		},
		curriculumLevelId: 'curriculum-version',
		expectedCount: 1,
		readyCount: 0,
		items: [item],
		extraOfferings: [],
		blockers: []
	};
}

test('a missing synchronized activity opens one preparation target across every matching program', () => {
	assert.equal(
		typeof deliveryModule.buildCurriculumPreparationTarget,
		'function',
		'the exact-focus curriculum preparation helper must exist'
	);
	const workspace = {
		academicTermId: 'term',
		academicYearId: 'year',
		deliveryVersionId: null,
		deliveryVersionStatus: null,
		deliveryVersionEffectiveFrom: null,
		homerooms: [
			room('ม.1/1', 'program-a', synchronizedItem),
			room('ม.1/2', 'program-a', { ...synchronizedItem, requirementId: 'club-requirement-b' }),
			room('ม.1/3', 'program-b', { ...synchronizedItem, requirementId: 'club-requirement-c' })
		],
		unlinked: []
	};

	const target = deliveryModule.buildCurriculumPreparationTarget(workspace, {
		resourceKind: 'activity',
		catalogVersionId: 'club-version'
	});

	assert.deepEqual(target, {
		resourceKind: 'activity',
		catalogVersionId: 'club-version',
		schedulingMode: 'synchronized',
		code: 'CLUB',
		name: 'ชุมนุม',
		studyProgramIds: ['program-a', 'program-b'],
		homeroomCount: 3
	});
});

test('focused preparation supports independent activities and excludes already opened entries', () => {
	const independent = { ...synchronizedItem, schedulingMode: 'independent' };
	assert.equal(
		deliveryModule.buildCurriculumPreparationTarget(
			{ homerooms: [room('ม.1/1', 'program-a', independent)] },
			{ resourceKind: 'activity', catalogVersionId: 'club-version' }
		).schedulingMode,
		'independent'
	);
	assert.equal(
		deliveryModule.buildCurriculumPreparationTarget(
			{ homerooms: [room('ม.1/1', 'program-a', { ...synchronizedItem, offeringId: 'offering' })] },
			{ resourceKind: 'activity', catalogVersionId: 'club-version' }
		),
		null
	);
});

test('pending opening preparation has no timetable inclusion action', () => {
	for (const mode of ['course', 'independent', 'synchronized']) {
		const item = {
			...synchronizedItem,
			resourceKind: mode === 'course' ? 'course' : 'activity',
			schedulingMode: mode === 'course' ? null : mode
		};
		const target = deliveryModule.buildCurriculumPreparationTarget(
			{ homerooms: [room('ม.1/1', 'program-a', item)] },
			{ resourceKind: item.resourceKind, catalogVersionId: item.catalogVersionId }
		);
		assert.equal(target.resourceKind, item.resourceKind);
		assert.equal(target.catalogVersionId, item.catalogVersionId);
		assert.equal(target.homeroomCount, 1);
		assert.equal(
			deliveryModule.buildCurriculumPreparationTarget(
				{ homerooms: [room('ม.1/1', 'program-a', { ...item, offeringId: 'offering' })] },
				{ resourceKind: item.resourceKind, catalogVersionId: item.catalogVersionId }
			),
			null
		);
	}
	assert.equal(
		deliveryModule.deliveryTimetableAction,
		undefined,
		'opening preparation never mutates a timetable version'
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

test('synchronized activities default to central activation but retain reviewed group alternatives', () => {
	const proposal = {
		proposalId: 'central',
		resourceKind: 'activity',
		schedulingMode: 'synchronized',
		catalogVersionId: 'club',
		groupingState: 'proposed',
		conflicts: [],
		defaultGroups: [{ groupKey: 'a', name: 'ชุมนุม', homeroomIds: ['a'] }]
	};
	assert.deepEqual(deliveryModule.buildFocusedCurriculumPreparationChoices([proposal], null), [
		{ proposalId: 'central', action: 'defer_groups', groups: [] }
	]);
	assert.equal(proposal.defaultGroups.length, 1);
	assert.equal(
		deliveryModule.buildFocusedCurriculumPreparationChoices(
			[{ ...proposal, schedulingMode: 'independent' }],
			null
		)[0].action,
		'apply'
	);
});

test('only drafts from the latest published opening are candidates, regardless of current historical view', () => {
	const old = { id: 'old', status: 'published', effectiveFrom: '2026-05-01' };
	const latest = { id: 'new', status: 'published', effectiveFrom: '2026-09-01' };
	const oldDraft = { id: 'old-draft', status: 'draft', sourceVersionId: old.id };
	const draft = { id: 'draft', status: 'draft', sourceVersionId: latest.id };
	assert.deepEqual(
		deliveryModule.deliveryPreparationDraftCandidates([oldDraft, old, latest, draft]),
		{
			source: latest,
			drafts: [draft]
		}
	);
	assert.deepEqual(deliveryModule.deliveryPreparationDraftCandidates([old, latest]), {
		source: latest,
		drafts: []
	});
	assert.equal(
		deliveryModule.deliveryPreparationDraftCandidates([
			old,
			latest,
			draft,
			{ ...draft, id: 'other' }
		]).drafts.length,
		2
	);
});

test('focused curriculum preparation supports exact course and independent activity versions without pairing same-code records', () => {
	const current = {
		...synchronizedItem,
		resourceKind: 'course',
		catalogVersionId: 'new-course-version',
		schedulingMode: null,
		code: 'ว30104',
		name: 'วิทยาศาสตร์'
	};
	const previous = {
		...current,
		catalogVersionId: 'old-course-version',
		offeringId: 'old-offering',
		name: 'วิทยาศาสตร์*'
	};
	const workspace = {
		homerooms: [
			room('room-a', 'program-a', current),
			room('room-b', 'program-b', current),
			room('room-c', 'program-c', previous)
		]
	};
	const target = deliveryModule.buildCurriculumPreparationTarget(workspace, {
		resourceKind: 'course',
		catalogVersionId: 'new-course-version'
	});
	assert.deepEqual(target.studyProgramIds, ['program-a', 'program-b']);
	assert.equal(target.homeroomCount, 2);
	assert.equal(target.schedulingMode, null);
	assert.equal(
		deliveryModule.buildCurriculumPreparationTarget(workspace, {
			resourceKind: 'course',
			catalogVersionId: 'old-course-version'
		}),
		null
	);
	const independent = { ...synchronizedItem, schedulingMode: 'independent' };
	assert.equal(
		deliveryModule.buildCurriculumPreparationTarget(
			{ homerooms: [room('room-a', 'program-a', independent)] },
			{ resourceKind: 'activity', catalogVersionId: 'club-version' }
		).schedulingMode,
		'independent'
	);
});

test('a partially opened exact version prepares only the still-missing room and program scope', () => {
	const offered = {
		...synchronizedItem,
		resourceKind: 'course',
		catalogVersionId: 'same-version',
		schedulingMode: null,
		offeringId: 'offering-a',
		offeringState: 'draft'
	};
	const pending = { ...offered, offeringId: null, offeringState: 'missing' };
	const target = deliveryModule.buildCurriculumPreparationTarget(
		{ homerooms: [room('room-a', 'program-a', offered), room('room-b', 'program-b', pending)] },
		{ resourceKind: 'course', catalogVersionId: 'same-version' }
	);
	assert.deepEqual(target.studyProgramIds, ['program-b']);
	assert.equal(target.homeroomCount, 1);
});
