import assert from 'node:assert/strict';
import test from 'node:test';
import {
	organizationPickerOptions,
	organizationUnitPath
} from '../../src/lib/features/staff/organization-hierarchy.ts';
import {
	organizationPositionOptions,
	organizationPositionLabel
} from '../../src/lib/features/staff/organization-positions.ts';

const unit = (id, name, parent_unit_id, display_order = 0) => ({
	id,
	name,
	name_en: null,
	parent_unit_id,
	display_order
});
const units = [
	unit('nested', 'งานประเมิน', 'division'),
	unit('division', 'งานทะเบียน', 'academic'),
	unit('budget-registration', 'งานทะเบียน', 'budget'),
	unit('budget', 'กลุ่มบริหารงบประมาณ', 'school', 2),
	unit('academic', 'กลุ่มบริหารวิชาการ', 'school', 1),
	unit('school', 'โรงเรียนตัวอย่าง', null),
	unit('orphan', 'งานเดิม', 'missing', 3),
	{ ...unit('subject', 'กลุ่มสาระภาษาไทย', 'school', 3), unit_type: 'subject_group' }
];

test('affiliation options include every level, learning area and orphan in tree order', () => {
	const before = structuredClone(units);
	assert.deepEqual(
		organizationPickerOptions(units).map((option) => option.unit.id),
		[
			'school',
			'academic',
			'division',
			'nested',
			'budget',
			'budget-registration',
			'subject',
			'orphan'
		]
	);
	assert.deepEqual(units, before);
});

test('search reaches the deepest unit and disambiguates equal names with the full ancestor path', () => {
	assert.deepEqual(
		organizationPickerOptions(units, 'งานประเมิน').map((option) => [
			option.unit.id,
			option.context
		]),
		[['nested', 'โรงเรียนตัวอย่าง › กลุ่มบริหารวิชาการ › งานทะเบียน']]
	);
	assert.deepEqual(
		organizationPickerOptions(units, 'งานทะเบียน').map((option) => [
			option.unit.id,
			option.context
		]),
		[
			['division', 'โรงเรียนตัวอย่าง › กลุ่มบริหารวิชาการ'],
			['nested', 'โรงเรียนตัวอย่าง › กลุ่มบริหารวิชาการ › งานทะเบียน'],
			['budget-registration', 'โรงเรียนตัวอย่าง › กลุ่มบริหารงบประมาณ']
		]
	);
	assert.equal(
		organizationUnitPath(units[0], units),
		'โรงเรียนตัวอย่าง › กลุ่มบริหารวิชาการ › งานทะเบียน › งานประเมิน'
	);
	assert.deepEqual(organizationPickerOptions(units, 'ไม่พบคำนี้'), []);
	assert.deepEqual(organizationPickerOptions([]), []);
});

test('all six shared position codes retain their Thai labels and existing grouping', () => {
	assert.deepEqual(
		organizationPositionOptions.map(({ value, label, group }) => [value, label, group]),
		[
			['director', 'ผู้อำนวยการ', 'leadership'],
			['deputy_director', 'รองผู้อำนวยการ', 'leadership'],
			['head', 'หัวหน้า', 'leadership'],
			['deputy_head', 'รองหัวหน้า', 'coordination'],
			['coordinator', 'ผู้ประสานงาน', 'coordination'],
			['member', 'สมาชิก', 'member']
		]
	);
	for (const option of organizationPositionOptions)
		assert.equal(organizationPositionLabel(option.value), option.label);
	assert.equal(organizationPositionLabel('custom-position'), 'custom-position');
});
