import assert from 'node:assert/strict';
import test from 'node:test';
import {
	moveCurriculumItem,
	orderedCurriculumPrograms,
	orderedCurriculumProgramOptions,
	rebaseAfterOwnProgramOrder
} from '../../src/lib/academic-core/curriculum-program-order.ts';

test('canonical custom order wins over names and legacy ties use code then ID without mutating input', () => {
	const programs = [
		{ id: 'z', code: 'B', displayOrder: 0, name: 'A' },
		{ id: 'b', code: 'A', displayOrder: 0, name: 'Z' },
		{ id: 'a', code: 'A', displayOrder: 0, name: 'Y' }
	];
	assert.deepEqual(
		orderedCurriculumPrograms(programs).map((item) => item.id),
		['a', 'b', 'z']
	);
	assert.deepEqual(
		orderedCurriculumPrograms([
			{ ...programs[0], displayOrder: 1 },
			{ ...programs[1], displayOrder: 2 }
		]).map((item) => item.id),
		['z', 'b']
	);
	assert.equal(programs[0].id, 'z');
});
test('ordinal movement preserves exact identities and invalid or boundary moves are no-ops', () => {
	const ids = ['a', 'b', 'c'];
	assert.deepEqual(moveCurriculumItem(ids, 2, 0), ['c', 'a', 'b']);
	assert.deepEqual(moveCurriculumItem(ids, 0, 2), ['b', 'c', 'a']);
	for (const [from, to] of [
		[-1, 0],
		[0, -1],
		[0, 3],
		[1, 1]
	])
		assert.deepEqual(moveCurriculumItem(ids, from, to), ids);
	assert.deepEqual(ids, ['a', 'b', 'c']);
});

test('option canonical ordering preserves level groups and never sorts programs by their names', () => {
	const programs = [
		{ id: 'b', code: 'B', displayOrder: 2, curriculumLevelId: 'level-1' },
		{ id: 'x', code: 'X', displayOrder: 1, curriculumLevelId: 'level-2' },
		{ id: 'a', code: 'A', displayOrder: 1, curriculumLevelId: 'level-1' }
	];
	assert.deepEqual(
		orderedCurriculumProgramOptions(programs).map((program) => program.id),
		['a', 'b', 'x']
	);
});

test('locked before versions advance only matching captured mutation baselines; GET review versions never rebase drafts', () => {
	const previous = [
		{ id: 'a', rowVersion: 1 },
		{ id: 'b', rowVersion: 1 }
	];
	const after = [
		{ id: 'a', displayOrder: 2, rowVersion: 2 },
		{ id: 'b', displayOrder: 1, rowVersion: 2 }
	];
	const baselines = { requirementVersions: { a: 1, b: 1 }, slotVersion: 4 };
	assert.deepEqual(rebaseAfterOwnProgramOrder(baselines, previous, after, 4, 5), {
		requirementVersions: { a: 2, b: 2 },
		slotVersion: 5
	});
	assert.deepEqual(
		rebaseAfterOwnProgramOrder(
			baselines,
			[{ id: 'a', rowVersion: 2 }, previous[1]],
			[{ ...after[0], rowVersion: 3 }, after[1]],
			4,
			5
		),
		{ requirementVersions: { a: 1, b: 2 }, slotVersion: 5 }
	);
	assert.deepEqual(rebaseAfterOwnProgramOrder(baselines, previous, after, 5, 6), {
		requirementVersions: { a: 2, b: 2 },
		slotVersion: 4
	});
	assert.deepEqual(
		rebaseAfterOwnProgramOrder(
			baselines,
			previous,
			previous.map((program) => ({ ...program, displayOrder: 0 })),
			4,
			4
		),
		baselines
	);
	assert.deepEqual(baselines, { requirementVersions: { a: 1, b: 1 }, slotVersion: 4 });
});
