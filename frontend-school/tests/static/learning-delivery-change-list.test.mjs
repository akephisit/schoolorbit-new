import assert from 'node:assert/strict';
import test from 'node:test';
import { buildLearningDeliveryChangeRows } from '../../src/lib/academic/learning-delivery-change-list.ts';

const diff = (resourceId, field = 'รายการเปิดสอน') => ({
	resourceId,
	learningOfferingId: 'offering',
	label: 'ชื่อซ้ำ',
	field,
	kind: 'changed',
	before: '1',
	after: '0'
});
const offeringItem = (id, learningOfferingId) => ({
	id,
	learningOfferingId,
	actionKind: 'adjust_weekly_period_target',
	weeklyPeriodTarget: 0,
	rowVersion: 3
});
const teacherItem = (id, learningGroupId) => ({
	id,
	learningGroupId,
	actionKind: 'stop_group_teacher',
	learningGroupLabel: 'กลุ่มทดสอบ',
	teacherLabel: 'ชื่อซ้ำ',
	rowVersion: 7
});

test('one offering keeps all differences and the exact command revision without mutating inputs', () => {
	const changes = [diff('offering'), diff('offering', 'ห้องที่เปิดสอน')];
	const item = offeringItem('period-command', 'offering');
	const items = [item];
	const before = structuredClone({ changes, items });
	const rows = buildLearningDeliveryChangeRows(changes, items);
	assert.equal(rows.length, 1);
	assert.deepEqual(rows[0].changes, changes);
	assert.equal(rows[0].items[0], item);
	assert.deepEqual({ changes, items }, before);
});

test('identical labels do not mix offering or teacher commands across stable IDs', () => {
	const rows = buildLearningDeliveryChangeRows(
		[diff('offering'), diff('group-a'), diff('group-b')],
		[
			teacherItem('teacher-a', 'group-a'),
			teacherItem('teacher-b', 'group-b'),
			offeringItem('periods', 'offering')
		]
	);
	assert.deepEqual(
		rows.map((row) => row.items.map((item) => item.id)),
		[['periods'], ['teacher-a'], ['teacher-b']]
	);
});

test('several teacher commands on one group remain individually removable', () => {
	const items = [
		teacherItem('stop-a', 'group'),
		{ ...teacherItem('add-b', 'group'), actionKind: 'add_group_teacher' }
	];
	const rows = buildLearningDeliveryChangeRows([diff('group')], items);
	assert.equal(rows.length, 1);
	assert.deepEqual(rows[0].items, items);
});

test('commands without differences remain accessible and direct edits gain no delete command', () => {
	const rows = buildLearningDeliveryChangeRows(
		[diff('direct-group-edit')],
		[offeringItem('no-op', 'offering')]
	);
	assert.deepEqual(
		rows.map((row) => [row.resourceId, row.changes.length, row.items.length]),
		[
			['direct-group-edit', 1, 0],
			['offering', 0, 1]
		]
	);
	assert.deepEqual(buildLearningDeliveryChangeRows([], []), []);
});

test('offering groups use stable ownership, retain unanchored teacher commands and search every child', async () => {
	const { groupLearningDeliveryChanges, filterLearningDeliveryChanges } =
		await import('../../src/lib/academic/learning-delivery-change-list.ts');
	const rows = buildLearningDeliveryChangeRows(
		[diff('group-a'), { ...diff('other'), learningOfferingId: 'other' }],
		[
			teacherItem('known', 'group-a'),
			teacherItem('orphan', 'unknown'),
			teacherItem('snapshot', 'group-b')
		]
	);
	const groups = groupLearningDeliveryChanges(
		rows,
		[
			{ id: 'offering', code: 'ค1', name: 'คณิตศาสตร์' },
			{ id: 'other', code: 'ค2', name: 'คณิตศาสตร์' }
		],
		[
			{
				id: 'offering',
				code: 'ค1',
				name: 'คณิตศาสตร์',
				kind: 'course',
				groups: [{ id: 'group-b' }]
			}
		]
	);
	assert.equal(groups.length, 3);
	assert.deepEqual(
		groups.find((group) => group.id === 'offering').rows.map((row) => row.resourceId),
		['group-a', 'group-b']
	);
	assert.equal(
		groups.find((group) => group.id === 'teacher-commands').rows[0].items[0].id,
		'orphan'
	);
	assert.equal(
		filterLearningDeliveryChanges(groups, 'ชื่อซ้ำ').flatMap((group) => group.rows).length,
		rows.length
	);
	assert.equal(filterLearningDeliveryChanges(groups, 'ค1')[0].rows.length, 2);
	assert.deepEqual(filterLearningDeliveryChanges(groups, 'ไม่มีคำนี้'), []);
});

test('bounded pages retain every resource and clamp after deletion without modifying source', async () => {
	const { deliveryChangePage } =
		await import('../../src/lib/academic/learning-delivery-change-list.ts');
	const items = Array.from({ length: 101 }, (_, index) => index);
	const pages = Array.from({ length: 13 }, (_, index) => deliveryChangePage(items, index));
	assert.ok(pages.every((page) => page.items.length <= 8));
	assert.deepEqual(
		pages.flatMap((page) => page.items),
		items
	);
	assert.deepEqual(deliveryChangePage([1], 12), { items: [1], page: 0, pageCount: 1, total: 1 });
	assert.deepEqual(deliveryChangePage([], 1).items, []);
	assert.equal(items.length, 101);
});
