import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const source = readFileSync(
	new URL('../../src/lib/academic-core/term-planning.ts', import.meta.url),
	'utf8'
);
const compiled = ts.transpileModule(source, {
	compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 }
}).outputText;
const { canPlanTermsInYear, termAnnualFlags } = await import(
	`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
);

test('future term planning is available only in planning and active years', () => {
	for (const status of ['planning', 'active']) assert.equal(canPlanTermsInYear(status), true);
	for (const status of ['ready', 'closing', 'closed', 'archived']) {
		assert.equal(canPlanTermsInYear(status), false);
	}
});

test('annual inclusion always requires closure while excluded terms retain an independent choice', () => {
	for (const included of [true, false]) {
		for (const blocks of [true, false]) {
			assert.deepEqual(termAnnualFlags(included, blocks), {
				includedInYearResult: included,
				blocksYearClosure: included || blocks
			});
		}
	}
});
