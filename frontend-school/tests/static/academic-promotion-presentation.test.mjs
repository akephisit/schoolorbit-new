import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const source = await readFile(
	new URL('../../src/lib/academic/lifecycle/promotion-presentation.ts', import.meta.url),
	'utf8'
);
const { outputText } = ts.transpileModule(source, {
	compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 }
});
const { initialDecision, decisionUsesDestination, decisionError, canApproveRun, canExecuteRun } =
	await import(`data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`);
const item = {
	status: 'reviewed',
	sourceGradeLevelId: 'old',
	decision: null,
	recommendation: {
		suggestedOutcome: 'promote',
		targetGradeLevelId: 'new',
		targetStudyProgramId: 'program',
		findings: []
	}
};

test('promotion draft suggests but does not invent a decision and does not mutate history', () => {
	const draft = initialDecision(item);
	assert.equal(draft.outcome, 'promote');
	assert.equal(draft.targetGradeLevelId, 'new');
	assert.equal(draft.targetStudyProgramId, 'program');
	assert.equal(
		initialDecision({ ...item, recommendation: { suggestedOutcome: null } }).outcome,
		''
	);
	const reviewed = { ...item, decision: { ...draft, reason: 'reviewed' } };
	initialDecision(reviewed).reason = 'edited';
	assert.equal(reviewed.decision.reason, 'reviewed');
});

test('promotion editor requires explicit targets reasons and conditional terms', () => {
	const draft = initialDecision(item);
	assert.equal(decisionError(draft, item), '');
	for (const outcome of ['promote', 'repeat', 'conditional'])
		assert.equal(decisionUsesDestination(outcome), true);
	for (const outcome of ['hold', 'graduate', 'transfer_out', ''])
		assert.equal(decisionUsesDestination(outcome), false);
	for (const invalid of [
		{ ...draft, outcome: '' },
		{ ...draft, targetGradeLevelId: null },
		{ ...draft, targetStudyProgramId: null },
		{ ...draft, targetGradeLevelId: 'old' },
		{ ...draft, outcome: 'repeat', reason: 'reason' },
		{ ...draft, outcome: 'conditional', reason: 'reason', condition: ' ' },
		{ ...draft, outcome: 'hold' },
		{ ...draft, reason: 'x'.repeat(1001) },
		{ ...draft, targetStudyProgramId: 'different', reason: ' ' }
	])
		assert.notEqual(decisionError(invalid, item), '');
	assert.equal(
		decisionError(
			{ ...draft, outcome: 'conditional', reason: 'reason', condition: 'follow up' },
			item
		),
		''
	);
	assert.equal(
		decisionError(
			{ ...draft, outcome: 'repeat', targetGradeLevelId: 'old', reason: 'repeat' },
			item
		),
		''
	);
	assert.equal(
		decisionError(
			{
				...draft,
				outcome: 'hold',
				targetGradeLevelId: null,
				targetStudyProgramId: null,
				reason: 'review later'
			},
			item
		),
		''
	);
});

test('promotion approval and execution remain separate and interrupted completion can resume', () => {
	const workspace = {
		run: { status: 'reviewed', approvalId: null },
		students: [
			{
				item: { ...item, decision: initialDecision(item) },
				annualResultCurrent: true,
				needsRecalculation: false
			}
		]
	};
	assert.equal(canApproveRun(workspace), true);
	assert.equal(canExecuteRun(workspace), false);
	assert.equal(canApproveRun({ ...workspace, students: [] }), false);
	assert.equal(
		canApproveRun({
			...workspace,
			students: [{ ...workspace.students[0], needsRecalculation: true }]
		}),
		false
	);
	for (const status of ['approved', 'executing', 'failed']) {
		const ready = { ...workspace, run: { status, approvalId: 'approval' } };
		assert.equal(canExecuteRun(ready), true);
		assert.equal(canApproveRun(ready), false);
		assert.equal(canExecuteRun({ ...ready, run: { status, approvalId: null } }), false);
	}
	assert.equal(
		canExecuteRun({
			...workspace,
			run: { status: 'executing', approvalId: 'approval' },
			students: [{ ...workspace.students[0], item: { ...item, status: 'executed' } }]
		}),
		true
	);
	assert.equal(
		canExecuteRun({ ...workspace, run: { status: 'completed', approvalId: 'approval' } }),
		false
	);
});
