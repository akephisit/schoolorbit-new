import assert from 'node:assert/strict';
import test from 'node:test';

import {
	clearGradebookItemSelection,
	nextEditableCell,
	normalizeScorePaste,
	selectAllGradebookItems,
	selectNewGradebookItem,
	toggleGradebookItemSelection
} from '../../src/lib/academic/gradebook/ledger.ts';

const itemIds = ['item-a', 'item-b', 'item-c'];
const studentIds = ['student-1', 'student-2', 'student-3'];

test('gradebook item selection starts empty, selects all, clears, and selects a new item', () => {
	assert.deepEqual(clearGradebookItemSelection(), []);
	assert.deepEqual(selectAllGradebookItems(itemIds), itemIds);
	assert.deepEqual(clearGradebookItemSelection(), []);
	assert.deepEqual(selectNewGradebookItem(['item-a'], 'item-d'), ['item-a', 'item-d']);
	assert.deepEqual(selectNewGradebookItem(['item-a'], 'item-a'), ['item-a']);
	assert.deepEqual(toggleGradebookItemSelection(['item-a'], 'item-b'), ['item-a', 'item-b']);
	assert.deepEqual(toggleGradebookItemSelection(['item-a', 'item-b'], 'item-a'), ['item-b']);
});

test('keyboard traversal visits selected columns only for Tab and Enter directions', () => {
	const selected = ['item-a', 'item-c'];

	assert.deepEqual(
		nextEditableCell(
			{ itemId: 'item-a', studentId: 'student-1' },
			selected,
			studentIds,
			'tab_forward'
		),
		{ itemId: 'item-c', studentId: 'student-1' }
	);
	assert.deepEqual(
		nextEditableCell(
			{ itemId: 'item-c', studentId: 'student-1' },
			selected,
			studentIds,
			'tab_forward'
		),
		{ itemId: 'item-a', studentId: 'student-2' }
	);
	assert.deepEqual(
		nextEditableCell(
			{ itemId: 'item-c', studentId: 'student-1' },
			selected,
			studentIds,
			'enter_down'
		),
		{ itemId: 'item-c', studentId: 'student-2' }
	);
	assert.deepEqual(
		nextEditableCell(
			{ itemId: 'item-a', studentId: 'student-1' },
			selected,
			studentIds,
			'tab_backward'
		),
		{ itemId: 'item-c', studentId: 'student-3' }
	);
	assert.deepEqual(
		nextEditableCell(
			{ itemId: 'item-a', studentId: 'student-1' },
			selected,
			studentIds,
			'enter_up'
		),
		{ itemId: 'item-c', studentId: 'student-3' }
	);
});

test('keyboard traversal returns null with no selected columns and skips an unchecked origin', () => {
	assert.equal(
		nextEditableCell({ itemId: 'item-a', studentId: 'student-1' }, [], studentIds, 'tab_forward'),
		null
	);
	assert.deepEqual(
		nextEditableCell(
			{ itemId: 'item-b', studentId: 'student-2' },
			['item-a', 'item-c'],
			studentIds,
			'tab_forward'
		),
		{ itemId: 'item-a', studentId: 'student-2' }
	);
});

test('score paste maps a bounded rectangle over selected columns and preserves explicit zero', () => {
	const result = normalizeScorePaste(
		'0\t2.5\n3\t',
		{ itemId: 'item-a', studentId: 'student-1' },
		['item-a', 'item-c'],
		studentIds
	);

	assert.equal(result.ok, true);
	if (!result.ok) return;
	assert.deepEqual(result.mutations, [
		{ itemId: 'item-a', studentId: 'student-1', value: '0' },
		{ itemId: 'item-c', studentId: 'student-1', value: '2.5' },
		{ itemId: 'item-a', studentId: 'student-2', value: '3' },
		{ itemId: 'item-c', studentId: 'student-2', value: null }
	]);
});

test('score paste treats a blank as clear and reports typed validation errors', () => {
	const blank = normalizeScorePaste(
		'',
		{ itemId: 'item-a', studentId: 'student-1' },
		['item-a'],
		studentIds
	);
	assert.deepEqual(blank, {
		ok: true,
		mutations: [{ itemId: 'item-a', studentId: 'student-1', value: null }]
	});

	for (const [text, expectedCode] of [
		['1\t2\n3', 'non_rectangular'],
		['1.234', 'invalid_decimal'],
		['-1', 'invalid_decimal'],
		['1\t2\t3', 'out_of_bounds']
	] as const) {
		const result = normalizeScorePaste(
			text,
			{ itemId: 'item-a', studentId: 'student-1' },
			['item-a', 'item-c'],
			studentIds
		);
		assert.equal(result.ok, false);
		if (!result.ok) assert.equal(result.error.code, expectedCode);
	}
});

test('score paste rejects a read-only origin, no selected columns, and an oversized batch', () => {
	const noColumns = normalizeScorePaste(
		'1',
		{ itemId: 'item-a', studentId: 'student-1' },
		[],
		studentIds
	);
	assert.equal(noColumns.ok, false);
	if (!noColumns.ok) assert.equal(noColumns.error.code, 'no_selected_columns');

	const readOnly = normalizeScorePaste(
		'1',
		{ itemId: 'item-b', studentId: 'student-1' },
		['item-a'],
		studentIds
	);
	assert.equal(readOnly.ok, false);
	if (!readOnly.ok) assert.equal(readOnly.error.code, 'origin_not_editable');

	const oversizedText = Array.from({ length: 101 }, () =>
		Array.from({ length: 10 }, () => '1').join('\t')
	).join('\n');
	const oversized = normalizeScorePaste(
		oversizedText,
		{ itemId: 'item-0', studentId: 'student-0' },
		Array.from({ length: 10 }, (_, index) => `item-${index}`),
		Array.from({ length: 101 }, (_, index) => `student-${index}`)
	);
	assert.equal(oversized.ok, false);
	if (!oversized.ok) assert.equal(oversized.error.code, 'too_many_cells');
});
