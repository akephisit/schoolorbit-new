import assert from 'node:assert/strict';
import test from 'node:test';
import {
	bahtInput,
	toSatang,
	quantityMillis,
	lineTotal,
	sumSatang,
	money
} from '../../src/lib/budget/money.ts';
import { BudgetMutationIntent } from '../../src/lib/budget/mutation-intent.ts';

test('budget money stays exact beyond safe browser integers', () => {
	assert.equal(toSatang('9007199254740993.25'), '900719925474099325');
	assert.equal(bahtInput('900719925474099325'), '9007199254740993.25');
	assert.equal(money('358800'), '3,588.00');
	assert.equal(sumSatang(['358800', '304000', '330000']), '992800');
	for (const input of ['-1', '1.001', '1e3', '', 'NaN']) assert.throws(() => toSatang(input));
});

test('quantity multiplication rounds half up once to integer satang', () => {
	assert.equal(quantityMillis('1.001'), 1001n);
	assert.equal(lineTotal('0.005', '100'), '1');
	assert.equal(lineTotal('0.004', '100'), '0');
	assert.equal(lineTotal('2', '179400'), '358800');
	assert.equal(lineTotal('1.125', '100'), '113');
	assert.equal(lineTotal('9007199254740993', '100'), '900719925474099300');
	for (const value of ['0', '-1', '1.0001', '1e2', '']) assert.throws(() => quantityMillis(value));
});

test('identical retries keep their key and changed payload starts a new intent', () => {
	const intent = new BudgetMutationIntent();
	const first = intent.for({ expectedRevision: 1, amountSatang: '358800' });
	assert.equal(intent.for({ expectedRevision: 1, amountSatang: '358800' }), first);
	assert.notEqual(intent.for({ expectedRevision: 2, amountSatang: '358800' }), first);
	intent.clear();
	assert.notEqual(intent.for({ expectedRevision: 1, amountSatang: '358800' }), first);
});
