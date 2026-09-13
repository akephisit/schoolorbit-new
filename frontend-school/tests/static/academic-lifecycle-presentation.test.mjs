import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const source = await readFile(
	new URL('../../src/lib/academic/lifecycle/presentation.ts', import.meta.url),
	'utf8'
);
const { outputText } = ts.transpileModule(source, {
	compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 }
});
const { canConfirmTransition } = await import(
	`data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`
);

const workspace = {
	context: { yearStartDate: '2026-05-01', yearEndDate: '2027-04-30', termStartDate: '2026-05-16' },
	coverage: { ready: true },
	availableActions: ['close', 'reopen', 'begin_closing'],
	findings: [{ code: 'exams.warning', severity: 'warning', count: 1 }]
};

test('term closure needs current capability, complete coverage, valid date and exact warning acknowledgements', () => {
	assert.equal(canConfirmTransition(workspace, 'close', '2026-09-30', '', ['exams.warning']), true);
	for (const date of [undefined, '', '2026-05-15', '2027-05-01', '2026-02-31', '2026-13-01']) {
		assert.equal(canConfirmTransition(workspace, 'close', date, '', ['exams.warning']), false);
	}
	for (const ack of [[], ['exams.warning', 'extra'], ['exams.warning', 'exams.warning']]) {
		assert.equal(canConfirmTransition(workspace, 'close', '2026-09-30', '', ack), false);
	}
	assert.equal(
		canConfirmTransition({ ...workspace, availableActions: [] }, 'close', '2026-09-30', '', [
			'exams.warning'
		]),
		false
	);
	assert.equal(
		canConfirmTransition({ ...workspace, coverage: { ready: false } }, 'close', '2026-09-30', '', [
			'exams.warning'
		]),
		false
	);
	assert.equal(
		canConfirmTransition(
			{ ...workspace, findings: [{ code: 'delivery.blocking', severity: 'blocking' }] },
			'close',
			'2026-09-30',
			'',
			[]
		),
		false
	);
});

test('begin closing does not require completed results; reopen needs a nonblank bounded reason', () => {
	const incomplete = { ...workspace, coverage: { ready: false } };
	assert.equal(canConfirmTransition(incomplete, 'begin_closing', undefined, '', []), true);
	assert.equal(canConfirmTransition(workspace, 'reopen', undefined, 'ทบทวนผล', []), true);
	for (const reason of ['', '  ', 'ก'.repeat(1001)]) {
		assert.equal(canConfirmTransition(workspace, 'reopen', undefined, reason, []), false);
	}
});
