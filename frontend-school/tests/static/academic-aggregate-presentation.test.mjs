import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const source = await readFile(
	new URL('../../src/lib/academic/results/aggregate-presentation.ts', import.meta.url),
	'utf8'
);
const { outputText } = ts.transpileModule(source, {
	compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 }
});
const { aggregateStatus, canLockAggregate } = await import(
	`data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`
);

test('aggregate status distinguishes missing revisions from stale retained history', () => {
	assert.equal(aggregateStatus({ revisionId: null, isCurrent: false }), 'missing');
	assert.equal(aggregateStatus({ revisionId: 'revision', isCurrent: false }), 'stale');
	assert.equal(aggregateStatus({ revisionId: 'revision', isCurrent: true }), 'current');
});

test('aggregate lock requires authority and current complete coverage; holds require policy and reason', () => {
	const preview = {
		canLock: true,
		blockers: [],
		holdFindings: [],
		policy: {
			id: 'policy',
			name: 'Reviewed policy',
			passingGrade: '1',
			minimumLearnerLevel: 1,
			allowReviewedHolds: false,
			approvedBy: 'actor',
			approvedAt: '2026-09-11T00:00:00Z'
		}
	};
	assert.equal(canLockAggregate(preview, '', true, false), true);
	assert.equal(canLockAggregate(preview, '', false, false), false);
	assert.equal(canLockAggregate(preview, '', true, true), false);
	assert.equal(canLockAggregate(null, '', true, false), false);
	assert.equal(canLockAggregate({ ...preview, canLock: false }, '', true, false), false);
	assert.equal(
		canLockAggregate({ ...preview, blockers: ['missing_course_results'] }, '', true, false),
		false
	);
	assert.equal(canLockAggregate(preview, 'unneeded reason', true, false), false);
	const held = { ...preview, holdFindings: ['exceptional_course_outcomes'] };
	assert.equal(canLockAggregate(held, 'reviewed', true, false), false);
	held.policy = { ...preview.policy, allowReviewedHolds: true };
	for (const reason of ['', '  ', 'ก'.repeat(1001)])
		assert.equal(canLockAggregate(held, reason, true, false), false);
	assert.equal(canLockAggregate(held, 'ติดตามผลค้าง', true, false), true);
});
