import assert from 'node:assert/strict';
import test from 'node:test';
import {
	captureCurriculumQuickAction,
	quickActionRequirements,
	stageCurriculumRequirements,
	programRequirementInputs
} from '../../src/lib/academic-core/curriculum-quick-actions.ts';
import { curriculumStructure, curriculumVersion, ids } from '../e2e/helpers/curriculum-fixtures.ts';

const context = { studyProgramId: ids.program, gradeLevelId: ids.grade, termSlotId: ids.term };
function fixture() {
	const workspace = curriculumStructure(curriculumVersion(ids.curriculumVersion, 'draft'));
	workspace.level.draftId = 'draft';
	const row = workspace.requirements[0];
	context.termSlotId = row.termSlotId;
	// Same-pin duplicates are defensive helper inputs, not reachable persisted requirements.
	workspace.requirements.push(
		{ ...row, id: 'duplicate', displayOrder: 9 },
		{ ...row, id: 'other-grade', gradeLevel: { ...row.gradeLevel, id: 'g2' }, termSlotId: 't2' },
		{ ...row, id: 'other-program', studyProgramId: 'b' }
	);
	workspace.programs[0].rowVersion = 8;
	return workspace;
}
test('defensive same-pin helper input edits preserve other grades, terms, pins and order', () => {
	const workspace = fixture();
	const baseline = captureCurriculumQuickAction(
		workspace,
		{ kind: 'edit', context, requirementId: 'duplicate' },
		true
	)!;
	const rows = quickActionRequirements(baseline, { requirementKind: 'elective' });
	assert.equal(baseline.rowVersion, 8);
	assert.equal(baseline.draftId, 'draft');
	assert.equal(rows.length, 3);
	assert.deepEqual(
		rows.map((row) => row.requirementKind),
		['required', 'elective', 'required']
	);
	assert.equal(rows[1].displayOrder, 9);
	assert.ok(
		rows.every((row) => row.catalogVersionId === workspace.requirements[0].catalogVersionId)
	);
	assert.equal(rows[2].gradeLevelId, 'g2');
	assert.equal(rows[2].termSlotId, 't2');
	assert.equal(workspace.requirements[1].requirementKind, 'required');
});
test('defensive same-pin helper removal resolves exact ID and isolates its snapshot', () => {
	const workspace = fixture();
	const baseline = captureCurriculumQuickAction(
		workspace,
		{ kind: 'remove', context, requirementId: ids.requirement },
		true
	)!;
	workspace.requirements[1].displayOrder = 99;
	const rows = quickActionRequirements(baseline, { requirementKind: 'required' });
	assert.equal(rows.length, 2);
	assert.equal(rows[0].displayOrder, 9);
});
test('denied, published, aggregate, missing and mismatched targets cannot capture a write baseline', () => {
	const workspace = fixture();
	assert.equal(
		captureCurriculumQuickAction(workspace, { kind: 'add-course', context }, false),
		null
	);
	assert.equal(
		captureCurriculumQuickAction(
			{ ...workspace, level: { ...workspace.level, draftId: null } },
			{ kind: 'add-course', context },
			true
		),
		null
	);
	for (const changed of [
		{ studyProgramId: 'missing' },
		{ gradeLevelId: 'all' },
		{ termSlotId: 'missing' }
	]) {
		assert.equal(
			captureCurriculumQuickAction(
				workspace,
				{ kind: 'add-course', context: { ...context, ...changed } },
				true
			),
			null
		);
	}
	for (const requirementId of ['missing', 'other-program', 'other-grade']) {
		assert.equal(
			captureCurriculumQuickAction(workspace, { kind: 'edit', context, requirementId }, true),
			null
		);
	}
});
test('add scopes current eligible selections and retains pinned existing course identities', () => {
	const workspace = fixture();
	const baseline = captureCurriculumQuickAction(workspace, { kind: 'add-course', context }, true)!;
	const option = {
		id: 'new-v2',
		catalogId: workspace.requirements[0].catalogId,
		resourceKind: 'course' as const,
		code: 'ค101',
		name: 'คณิตศาสตร์',
		gradeLevelIds: [ids.grade],
		section: 'basic_course' as const,
		versionNo: 2,
		effectiveFrom: '2026-01-01',
		selectableForNewRequirement: true
	};
	const rows = quickActionRequirements(baseline, {
		requirementKind: 'optional',
		selectedIds: ['new-v2', 'new', 'activity', 'wrong-grade', 'retired'],
		options: [
			option,
			{ ...option, id: 'new', catalogId: 'new' },
			{ ...option, id: 'activity', resourceKind: 'activity' },
			{ ...option, id: 'wrong-grade', gradeLevelIds: ['g2'] },
			{ ...option, id: 'retired', selectableForNewRequirement: false }
		]
	});
	assert.equal(rows.length, 4);
	assert.equal(rows[3].catalogVersionId, 'new');
	assert.equal(rows[3].displayOrder, 10);
	assert.deepEqual(
		rows.slice(0, 3),
		programRequirementInputs(stageCurriculumRequirements(workspace.requirements), ids.program)
	);
});

test('menu state proxies are converted to plain action context before snapshot cloning', () => {
	const workspace = fixture();
	const action = new Proxy(
		{ kind: 'edit' as const, context: new Proxy({ ...context }, {}), requirementId: 'duplicate' },
		{}
	);
	const baseline = captureCurriculumQuickAction(workspace, action, true)!;
	assert.equal(baseline.action.context.studyProgramId, context.studyProgramId);
	assert.equal(baseline.target?.id, 'duplicate');
	assert.doesNotThrow(() => structuredClone(baseline));
});
