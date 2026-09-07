import assert from 'node:assert/strict';
import test from 'node:test';
import { scoreItemBudget } from '../../src/lib/academic/gradebook/item-budget.ts';

const item = (id: string, maxScore: string, lifecycle = 'active') => ({ id, maxScore, lifecycle });

test('allocation uses exact hundredths and ignores cancelled items', () => {
	assert.deepEqual(
		scoreItemBudget('0.30', [item('a', '0.10'), item('b', '0.20'), item('c', '5', 'cancelled')]),
		{
			allocated: 0.3,
			remaining: 0,
			maximumForItem: 0,
			canAdd: false
		}
	);
	assert.equal(scoreItemBudget('20', [item('a', '12.75')]).maximumForItem, 7.25);
});

test('editing credits the old maximum and permits gradual repair without increasing excess', () => {
	const items = [item('a', '10'), item('b', '10'), item('c', '10')];
	assert.deepEqual(scoreItemBudget('20', items, 'a'), {
		allocated: 30,
		remaining: -10,
		maximumForItem: 10,
		canAdd: false
	});
	assert.equal(scoreItemBudget('40', items, 'a').maximumForItem, 20);
});

test('zero maximum phases do not permit adding even zero-point items', () => {
	assert.equal(scoreItemBudget('0', []).canAdd, false);
});
