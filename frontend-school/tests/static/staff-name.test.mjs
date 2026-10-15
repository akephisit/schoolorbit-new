import assert from 'node:assert/strict';
import test from 'node:test';
import {
	staffNameDraft,
	staffNameErrors,
	preferredStaffName,
	staffNameInput
} from '../../src/lib/forms/staff-name.ts';
test('preferred identity uses a complete language and retains raw fields', () => {
	const name = {
		...staffNameDraft(),
		title: 'นาย',
		first_name: 'สมชาย',
		last_name: '',
		title_en: ' Mr. ',
		first_name_en: ' John ',
		last_name_en: ' Smith '
	};
	assert.equal(preferredStaffName(name), 'Mr. John Smith');
	assert.deepEqual(staffNameErrors(name), {});
	assert.equal(staffNameInput(name).first_name, 'สมชาย');
	assert.equal(staffNameInput(name).first_name_en, 'John');
	name.last_name = 'ทดสอบ';
	assert.equal(preferredStaffName(name), 'นายสมชาย ทดสอบ');
	name.first_name_en = '';
	name.title = '';
	assert.ok(staffNameErrors(name).names);
});
test('name limits count Unicode scalars and reject control characters before trimming', () => {
	const name = {
		...staffNameDraft(),
		title_en: 'Dr.',
		first_name_en: '😀'.repeat(100),
		last_name_en: 'Example'
	};
	assert.deepEqual(staffNameErrors(name), {});
	name.first_name_en += '😀';
	assert.ok(staffNameErrors(name).first_name_en);
	name.first_name_en = 'Alice\n';
	assert.ok(staffNameErrors(name).first_name_en);
});
