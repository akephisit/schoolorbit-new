import assert from 'node:assert/strict';
import test from 'node:test';
import { curriculumPreparationOrder } from '../../src/lib/academic/curriculum-preparation-order.ts';
import type { DeliveryManagementOptions } from '../../src/lib/api/learning-delivery.ts';

const options: Pick<DeliveryManagementOptions, 'homerooms' | 'gradeLevels'> = {
	gradeLevels: [
		{
			id: 'g2',
			code: 'M2',
			name: 'ม.2',
			short_name: 'ม.2',
			level_order: 302,
			level_type: 'secondary'
		},
		{
			id: 'g1',
			code: 'M1',
			name: 'ม.1',
			short_name: 'ม.1',
			level_order: 301,
			level_type: 'secondary'
		}
	],
	homerooms: [
		{ id: 'a', name: 'ม.2/1', gradeLevelId: 'g2' },
		{ id: 'b', name: 'ม.1/10', gradeLevelId: 'g1' },
		{ id: 'c', name: 'ม.1/2', gradeLevelId: 'g1' },
		{ id: 'd', name: 'ม.1/1', gradeLevelId: 'g1' }
	]
};
const group = (groupKey: string, homeroomIds: string[], name = groupKey) => ({
	groupKey,
	homeroomIds,
	name
});

test('room order uses grade metadata then natural Thai names without mutating canonical IDs', () => {
	const order = curriculumPreparationOrder(options);
	const input = ['a', 'b', 'c', 'd'];
	assert.deepEqual(order.homeroomIds(input), ['d', 'c', 'b', 'a']);
	assert.deepEqual(input, ['a', 'b', 'c', 'd']);
	const renamed = {
		...options,
		homerooms: options.homerooms.map((room) => ({
			...room,
			name: room.id === 'a' ? 'ห้อง 1' : room.name
		}))
	};
	assert.deepEqual(curriculumPreparationOrder(renamed).homeroomIds(input), ['d', 'c', 'b', 'a']);
});

test('groups anchor on lowest associated room and keep stable ties regardless of editable names or keys', () => {
	const input = [
		group('a', ['a']),
		group('z', ['b', 'd'], 'ท้าย'),
		group('b', ['d'], 'แรก'),
		group('c', ['c'])
	];
	const before = structuredClone(input);
	const order = curriculumPreparationOrder(options);
	assert.deepEqual(
		order.groups(input).map((item) => item.groupKey),
		['z', 'b', 'c', 'a']
	);
	input[1].name = 'ชื่อใหม่';
	assert.deepEqual(
		order.groups(input).map((item) => item.groupKey),
		['z', 'b', 'c', 'a']
	);
	assert.deepEqual(
		input.map((item) => item.homeroomIds),
		before.map((item) => item.homeroomIds)
	);
	assert.deepEqual(
		input.map((item) => item.groupKey),
		before.map((item) => item.groupKey)
	);
	assert.equal(order.groups(input)[0], input[1]);
});

test('missing grades use natural names and unknown or empty associations remain deterministic and retained', () => {
	const order = curriculumPreparationOrder({ homerooms: options.homerooms, gradeLevels: [] });
	assert.deepEqual(order.homeroomIds(['a', 'b', 'c', 'd']), ['d', 'c', 'b', 'a']);
	const input = [
		group('empty', []),
		group('unknown10', ['missing10']),
		group('known', ['a']),
		group('unknown2', ['missing2']),
		group('empty2', [])
	];
	assert.deepEqual(
		order.groups(input).map((item) => item.groupKey),
		['known', 'unknown2', 'unknown10', 'empty', 'empty2']
	);
	assert.equal(order.groups(input).length, input.length);
});
