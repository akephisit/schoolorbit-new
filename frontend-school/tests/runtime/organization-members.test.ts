import assert from 'node:assert/strict';
import test from 'node:test';
import { organizationPeople, organizationPrimaryError, setOrganizationPrimary } from '../../src/lib/features/staff/organization-members.ts';
import type { OrganizationMemberItem, OrganizationUnit } from '../../src/lib/api/staff.ts';
const assignment = (unit: string, user: string, position = 'head'): OrganizationMemberItem => ({
	organization_unit_id: unit, organization_unit_name: unit, user_id: user, name: user,
	title: '', position_code: position, position_title: 'ตำแหน่งต้นทาง', is_primary: true,
	responsibilities: null, started_at: '2026-01-01'
});
const unit = (id: string, unit_type: string): OrganizationUnit => ({
	id, unit_type, name: id, code: id, name_en: null, parent_unit_id: null,
	category: 'general', display_order: 0, is_active: true, is_system: false,
	description: null, phone: null, email: null, location: null, subject_group_id: null,
	created_at: '', updated_at: ''
});

test('one parent person preserves every source and direct role wins over inherited head roles', () => {
	const rows = [assignment('deep', 'one'), assignment('parent', 'one', 'member'), assignment('child', 'one'), assignment('deep', 'two')];
	const before = structuredClone(rows);
	const people = organizationPeople(rows, 'parent');
	assert.equal(people.length, 2);
	assert.equal(people[0].user_id, 'one');
	assert.equal(people[0].position_code, 'member');
	assert.equal(people[0].isDirect, true);
	assert.deepEqual(people[0].sourceAssignments.map((source) => source.organization_unit_id), ['parent', 'deep', 'child']);
	assert.equal(people[1].position_code, 'member');
	assert.equal(people[1].position_title, null);
	assert.equal(people[1].is_primary, false);
	assert.equal(people[1].sourceAssignments[0].position_code, 'head');
	assert.deepEqual(rows, before);
});

test('primary administration and learning area selections remain independent, activity divisions remain administration', () => {
	const units = [unit('admin', 'management_group'), unit('work', 'division'), unit('thai', 'subject_group'), unit('math', 'subject_group'), unit('activity', 'division')];
	const rows = units.map((item) => ({ organization_unit_id: item.id, is_primary: ['admin', 'thai'].includes(item.id) }));
	const changedArea = setOrganizationPrimary(rows, 3, units);
	assert.deepEqual(changedArea.map((item) => item.is_primary), [true, false, false, true, false]);
	const changedAdmin = setOrganizationPrimary(changedArea, 4, units);
	assert.deepEqual(changedAdmin.map((item) => item.is_primary), [false, false, false, true, true]);
	assert.equal(organizationPrimaryError(changedAdmin, units), '');
	assert.match(organizationPrimaryError(rows.map((item) => ({ ...item, is_primary: true })), units), /บริหาร/);
	assert.match(organizationPrimaryError(rows.map((item) => ({ ...item, is_primary: item.organization_unit_id === 'admin' })), units), /กลุ่มสาระ/);
});
