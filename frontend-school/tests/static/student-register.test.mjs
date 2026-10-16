import assert from 'node:assert/strict';
import test from 'node:test';
import {
	studentRegisterDraft,
	studentParentDraft,
	studentPasswordError,
	studentRegisterErrors,
	studentRegisterParents,
	createStudentRegisterInput,
	updateStudentRegisterInput,
	studentRegisterReadScope
} from '../../src/lib/forms/student-register.ts';

function validCreate() {
	return {
		...studentRegisterDraft(),
		student_id: '00123',
		first_name: 'นักเรียน',
		last_name: 'ทดสอบ',
		password: 'password123',
		confirmPassword: 'password123'
	};
}

test('permanent drafts preserve code and identity without copying credentials or annual fields', () => {
	const draft = studentRegisterDraft({ student_id: '00001', title: null, first_name: 'ชื่อ' });
	assert.equal(draft.student_id, '00001');
	assert.equal(draft.title, '');
	assert.equal(draft.first_name, 'ชื่อ');
	for (const field of ['grade_level', 'homeroom', 'student_number', 'status'])
		assert.equal(field in draft, false);
	assert.equal(draft.password, '');
	assert.equal(draft.national_id, '');
	assert.deepEqual(draft.parents, []);
});

test('password constraints count Unicode scalars and UTF-8 bytes without trimming', () => {
	assert.ok(studentPasswordError('a'.repeat(7)));
	assert.equal(studentPasswordError('a'.repeat(8)), null);
	assert.equal(studentPasswordError('a'.repeat(71)), null);
	assert.ok(studentPasswordError('a'.repeat(72)));
	assert.ok(studentPasswordError('a'.repeat(129)));
	assert.ok(studentPasswordError('😀'.repeat(7)));
	assert.equal(studentPasswordError('😀'.repeat(8)), null);
	assert.equal(studentPasswordError('ก'.repeat(23)), null);
	assert.ok(studentPasswordError('ก'.repeat(24)));
	assert.equal(studentPasswordError(' aaaaaa '), null);
});

test('hidden guardian drafts neither validate nor submit after being disabled', () => {
	const draft = { ...validCreate(), parents: [studentParentDraft()] };
	assert.deepEqual(studentRegisterErrors(draft, 'create'), {});
	assert.deepEqual(studentRegisterParents(draft), []);
	draft.parentsEnabled = true;
	assert.ok(studentRegisterErrors(draft, 'create')['parents.0.first_name']);
	draft.parentsEnabled = false;
	assert.deepEqual(studentRegisterErrors(draft, 'create'), {});
	assert.deepEqual(studentRegisterParents(draft), []);
});

test('create-only validation does not block edits and optional dates reject normalization', () => {
	const draft = { ...studentRegisterDraft(), first_name: 'ชื่อ', last_name: 'นามสกุล' };
	assert.deepEqual(studentRegisterErrors(draft, 'edit'), {});
	assert.ok(studentRegisterErrors(draft, 'create').password);
	draft.first_name = '  ';
	draft.email = 'invalid';
	draft.date_of_birth = '2025-02-29';
	const errors = studentRegisterErrors(draft, 'edit');
	assert.ok(errors.first_name);
	assert.ok(errors.email);
	assert.ok(errors.date_of_birth);
	draft.date_of_birth = '2024-02-29';
	assert.equal(studentRegisterErrors(draft, 'edit').date_of_birth, undefined);
});

test('enabled guardian normalization is explicit and does not mutate the active draft', () => {
	const draft = validCreate();
	draft.parentsEnabled = true;
	draft.parents = [
		{
			...studentParentDraft(),
			first_name: ' ผู้ปกครอง ',
			last_name: ' ทดสอบ ',
			phone: ' 0000000000 '
		}
	];
	assert.deepEqual(studentRegisterErrors(draft, 'create'), {});
	const parents = studentRegisterParents(draft);
	assert.equal(parents[0].first_name, 'ผู้ปกครอง');
	assert.equal(parents[0].phone, '0000000000');
	assert.equal(draft.parents[0].first_name, ' ผู้ปกครอง ');
});

test('create and edit payloads preserve permanent ownership and explicit clearing', () => {
	const draft = validCreate();
	draft.phone = ' +66 123 ';
	draft.national_id = '0000000000000';
	draft.parents = [studentParentDraft()];
	const create = createStudentRegisterInput(draft, false);
	assert.equal(create.student_id, '00123');
	assert.equal(create.phone, '+66 123');
	assert.equal(create.national_id, undefined);
	assert.equal(create.parents, undefined);
	assert.equal(create.title, undefined);
	const edit = updateStudentRegisterInput(draft);
	for (const field of [
		'student_id',
		'student_number',
		'grade_level',
		'homeroom',
		'national_id',
		'password',
		'confirmPassword',
		'parents',
		'parentsEnabled'
	])
		assert.equal(field in edit, false);
	for (const field of ['title', 'date_of_birth', 'gender', 'email', 'address', 'enrollment_date'])
		assert.equal(edit[field], null);
	assert.equal(edit.registration_status, 'pending');
	assert.equal(studentRegisterDraft({ first_name: 'legacy' }).registration_status, 'unrecorded');
});

test('field bounds match Unicode lengths and accept ordinary student phone text', () => {
	const draft = validCreate();
	draft.first_name = '😀'.repeat(100);
	draft.phone = '+66 (0) 123';
	draft.enrollment_date = '2024-02-29';
	assert.deepEqual(studentRegisterErrors(draft, 'create'), {});
	draft.first_name += '😀';
	draft.student_id = 'a'.repeat(51);
	draft.title = 'a'.repeat(51);
	draft.phone = 'a'.repeat(21);
	draft.address = 'a'.repeat(2001);
	draft.gender = 'unknown';
	draft.enrollment_date = '2025-02-29';
	draft.parentsEnabled = true;
	draft.parents = Array.from({ length: 11 }, studentParentDraft);
	const errors = studentRegisterErrors(draft, 'create');
	for (const field of [
		'first_name',
		'student_id',
		'title',
		'phone',
		'address',
		'gender',
		'enrollment_date',
		'parents'
	])
		assert.ok(errors[field], field);
});

test('read ownership distinguishes scope downgrades even when general read and PII access remain true', () => {
	const school = {
		school: true,
		assigned: false,
		own: false,
		piiSchool: true,
		piiAssigned: false,
		piiOwn: false
	};
	const assigned = { ...school, school: false, assigned: true };
	const ownPii = { ...school, piiSchool: false, piiOwn: true };
	assert.notEqual(studentRegisterReadScope(school), studentRegisterReadScope(assigned));
	assert.notEqual(studentRegisterReadScope(school), studentRegisterReadScope(ownPii));
	assert.equal(studentRegisterReadScope(school), studentRegisterReadScope({ ...school }));
});
