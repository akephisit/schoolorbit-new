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
