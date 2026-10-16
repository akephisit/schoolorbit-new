import assert from 'node:assert/strict';
import test from 'node:test';
import {
	studentRegisterContext,
	studentRegistryHref,
	withStudentRegisterContext
} from '../../src/lib/navigation/student-register.ts';

const year = '55000000-0000-4000-8000-000000000001';
const context = (query = '') =>
	studentRegisterContext(new URL(`https://school.test/staff/students/id${query}`));

test('registry context ignores stale year and account-style legacy filters', () => {
	assert.deepEqual(context('?academicYearId=year'), {
		returnTo: '/staff/students',
		academicYearId: null,
		source: 'registry'
	});
	assert.equal(
		studentRegistryHref(
			new URL(
				'https://school.test/staff/students?academicYearId=old&status=active&search=name&page=2&registrationStatus=graduated&accountStatus=inactive'
			)
		),
		'/staff/students?search=name&registrationStatus=graduated&accountStatus=inactive&page=2'
	);
});

test('registry filters survive detail and edit navigation without creating annual context', () => {
	const returnTo = '/staff/students?search=ชื่อ&page=2&registrationStatus=graduated';
	const original = context(`?academicYearId=old&returnTo=${encodeURIComponent(returnTo)}`);
	assert.equal(original.academicYearId, null);
	const editHref = withStudentRegisterContext('/staff/students/id/edit', original);
	const next = studentRegisterContext(new URL(editHref, 'https://school.test'));
	assert.deepEqual(next, original);
	assert.equal(new URL(next.returnTo, 'https://school.test').searchParams.get('search'), 'ชื่อ');
});

test('annual and homeroom sources require matching explicit years', () => {
	for (const path of [
		'/staff/academic/student-years',
		'/staff/academic/homerooms/room-id/students'
	]) {
		const returnTo = `${path}?academicYearId=${year}`;
		const selected = context(`?academicYearId=${year}&returnTo=${encodeURIComponent(returnTo)}`);
		assert.equal(selected.academicYearId, year);
		assert.equal(selected.returnTo, returnTo);
		assert.notEqual(selected.source, 'registry');
		assert.equal(
			context(`?academicYearId=other&returnTo=${encodeURIComponent(returnTo)}`).academicYearId,
			null
		);
	}
});

test('external, malformed, unrelated and nested return destinations fall back safely', () => {
	for (const destination of [
		'https://evil.test',
		'//evil.test',
		'/\\evil.test',
		'/staff/manage',
		'/staff/students#fragment',
		'/staff/academic/student-years?academicYearId=year&returnTo=//evil.test',
		'http://['
	]) {
		assert.equal(
			context(`?academicYearId=year&returnTo=${encodeURIComponent(destination)}`).returnTo,
			'/staff/students'
		);
	}
});
