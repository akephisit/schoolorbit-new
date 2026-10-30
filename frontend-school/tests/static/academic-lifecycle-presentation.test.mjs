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
	context: {},
	coverage: { ready: true },
	availableActions: ['close', 'reopen', 'begin_closing'],
	findings: [{ code: 'exams.warning', severity: 'warning', count: 1 }]
};

test('term closure needs current capability, complete coverage, exact warning acknowledgements', () => {
	assert.equal(canConfirmTransition(workspace, 'close', '', ['exams.warning']), true);
	for (const ack of [[], ['exams.warning', 'extra'], ['exams.warning', 'exams.warning']]) {
		assert.equal(canConfirmTransition(workspace, 'close', '', ack), false);
	}
	assert.equal(
		canConfirmTransition({ ...workspace, availableActions: [] }, 'close', '', ['exams.warning']),
		false
	);
	assert.equal(
		canConfirmTransition({ ...workspace, coverage: { ready: false } }, 'close', '', [
			'exams.warning'
		]),
		false
	);
	assert.equal(
		canConfirmTransition(
			{ ...workspace, findings: [{ code: 'delivery.blocking', severity: 'blocking' }] },
			'close',
			'',
			[]
		),
		false
	);
});

test('begin closing does not require completed results; reopen needs a nonblank bounded reason', () => {
	const incomplete = { ...workspace, coverage: { ready: false } };
	assert.equal(canConfirmTransition(incomplete, 'begin_closing', '', []), true);
	assert.equal(canConfirmTransition(workspace, 'reopen', 'ทบทวนผล', []), true);
	for (const reason of ['', '  ', 'ก'.repeat(1001)]) {
		assert.equal(canConfirmTransition(workspace, 'reopen', reason, []), false);
	}
});
