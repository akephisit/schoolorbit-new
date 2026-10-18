import test from 'node:test';
import assert from 'node:assert/strict';
import {
	recipientLabel,
	mergeRecipientLookups,
	recipientKey
} from '../../src/lib/correspondence/recipients.ts';

test('review labels retain previous search names and exact logical recipient kinds', () => {
	const first = {
		templates: [],
		users: [{ id: 'one', displayName: 'ครูหนึ่ง' }],
		organizations: [{ id: 'unit', name: 'งานวิชาการ' }],
		positions: [{ organizationUnitId: 'unit', code: 'HEAD', name: 'หัวหน้างาน' }]
	};
	const second = {
		templates: [],
		users: [{ id: 'two', displayName: 'ครูสอง' }],
		organizations: [],
		positions: []
	};
	const merged = mergeRecipientLookups(first, second);
	assert.equal(recipientLabel({ kind: 'user', userId: 'one' }, merged), 'ครูหนึ่ง');
	assert.equal(recipientLabel({ kind: 'user', userId: 'two' }, merged), 'ครูสอง');
	assert.equal(
		recipientLabel({ kind: 'organization_unit', organizationUnitId: 'unit' }, merged),
		'หน่วยงาน งานวิชาการ'
	);
	assert.equal(
		recipientLabel(
			{ kind: 'organization_position', organizationUnitId: 'unit', positionCode: 'HEAD' },
			merged
		),
		'งานวิชาการ · หัวหน้างาน'
	);
	assert.notEqual(
		recipientKey({ kind: 'organization_unit', organizationUnitId: 'unit' }),
		recipientKey({
			kind: 'organization_position',
			organizationUnitId: 'unit',
			positionCode: 'HEAD'
		})
	);
	assert.equal(first.users.length, 1);
});
