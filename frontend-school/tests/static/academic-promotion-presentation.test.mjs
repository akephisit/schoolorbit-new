import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';
const source = await readFile(
	new URL('../../src/lib/academic/lifecycle/year-management.ts', import.meta.url),
	'utf8'
);
const { outputText } = ts.transpileModule(source, {
	compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 }
});
const {
	initialRolloverDraft,
	changeOutcome,
	rosterDecisions,
	rosterDraftError,
	sourceRoomKey,
	academicStatusLabel
} = await import(`data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`);
const student = {
	studentAcademicYearId: 'annual-id',
	studentId: 'same-person',
	studentName: 'นักเรียน',
	sourceGradeLevelId: 'P6',
	sourceStudyProgramId: 'primary',
	sourceHomeroomId: 'old-room',
	suggestedDecision: null,
	decision: null
};
const preview = {
	students: [student],
	programs: [
		{ id: 'secondary', gradeLevelIds: ['M1'] },
		{ id: 'primary', gradeLevelIds: ['P6'] }
	],
	homerooms: [{ id: 'new-room', gradeLevelId: 'M1', studyProgramId: 'secondary' }]
};
test('final grades require explicit choices and retain source identity', () => {
	const draft = initialRolloverDraft(student);
	assert.equal(draft.outcome, '');
	assert.notEqual(rosterDraftError(preview, { 'annual-id': draft }), '');
	const decision = {
		outcome: 'promote',
		targetGradeLevelId: 'M1',
		targetStudyProgramId: 'secondary',
		targetHomeroomId: null,
		reason: null,
		condition: null
	};
	assert.equal(rosterDraftError(preview, { 'annual-id': decision }), '');
	assert.equal(
		rosterDecisions([student], { 'annual-id': decision })[0].studentAcademicYearId,
		'annual-id'
	);
	assert.equal(student.studentId, 'same-person');
});
test('repeat uses original grade, while non-destination outcomes clear placement fields', () => {
	const draft = {
		outcome: 'promote',
		targetGradeLevelId: 'M1',
		targetStudyProgramId: 'secondary',
		targetHomeroomId: 'new-room'
	};
	assert.equal(changeOutcome(draft, 'repeat', student).targetGradeLevelId, 'P6');
	for (const outcome of ['hold', 'graduate', 'transfer_out']) {
		const next = changeOutcome(draft, outcome, student);
		assert.equal(next.targetGradeLevelId, null);
		assert.equal(next.targetStudyProgramId, null);
		assert.equal(next.targetHomeroomId, null);
		assert.equal(rosterDraftError(preview, { 'annual-id': next }), '');
	}
});
test('destination room must match grade and program, and unassigned rooms remain explicit', () => {
	const valid = {
		outcome: 'promote',
		targetGradeLevelId: 'M1',
		targetStudyProgramId: 'secondary',
		targetHomeroomId: null
	};
	assert.equal(rosterDraftError(preview, { 'annual-id': valid }), '');
	assert.notEqual(
		rosterDraftError(preview, { 'annual-id': { ...valid, targetHomeroomId: 'old-room' } }),
		''
	);
	assert.equal(
		rosterDraftError(preview, { 'annual-id': { ...valid, targetHomeroomId: 'new-room' } }),
		''
	);
	assert.notEqual(
		sourceRoomKey(student),
		sourceRoomKey({ ...student, sourceHomeroomId: 'other-room' })
	);
});
test('historical intermediate statuses use only the three simple labels', () => {
	assert.equal(academicStatusLabel('ready'), 'เตรียมการ');
	assert.equal(academicStatusLabel('closing'), 'ใช้งาน');
	assert.equal(academicStatusLabel('archived'), 'ปิดแล้ว');
});

test('accepted conditional destinations carry exact historical intent without becoming a new choice', () => {
	const decision = {
		outcome: 'conditional',
		targetGradeLevelId: 'M1',
		targetStudyProgramId: 'secondary',
		targetHomeroomId: 'new-room',
		reason: ' เหตุผลเดิม ',
		condition: ' เงื่อนไขเดิม '
	};
	const retained = {
		...student,
		existingTargetStudentYearId: 'prior-target',
		suggestedDecision: decision
	};
	const draft = initialRolloverDraft(retained);
	assert.deepEqual(draft.retainedDecision, decision);
	assert.deepEqual(rosterDecisions([retained], { 'annual-id': draft })[0].decision, decision);
	assert.equal(rosterDraftError({ ...preview, students: [retained] }, { 'annual-id': draft }), '');
	assert.deepEqual(changeOutcome(draft, 'hold', retained), draft);
	assert.equal(initialRolloverDraft({ ...student, suggestedDecision: decision }).outcome, '');
});
