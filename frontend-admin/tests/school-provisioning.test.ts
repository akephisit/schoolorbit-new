import assert from 'node:assert/strict';
import test from 'node:test';

import { schoolCreationFailure } from '../src/lib/utils/school-provisioning.ts';

test('failed school creation removes only its temporary card', () => {
	const schools = [
		{ id: 'existing-school', name: 'Existing' },
		{ id: 'temporary-school', name: 'Temporary' }
	];

	const result = schoolCreationFailure(schools, 'temporary-school', 'Connection lost');

	assert.deepEqual(result.schools, [{ id: 'existing-school', name: 'Existing' }]);
});

test('Neon provider details become a safe actionable message', () => {
	const rawError =
		'External Service Error: Failed to create database: Neon API error (404 Not Found): ' +
		'{"request_id":"provider-request-secret-detail","message":"branch not found"}';

	const result = schoolCreationFailure([], 'temporary-school', rawError);

	assert.equal(
		result.message,
		'ไม่สามารถสร้างฐานข้อมูลโรงเรียนได้ กรุณาตรวจสอบการตั้งค่า Neon แล้วลองอีกครั้ง'
	);
	assert.doesNotMatch(result.message, /provider-request-secret-detail|branch not found/i);
});

test('duplicate subdomain remains actionable after sanitizing backend errors', () => {
	const result = schoolCreationFailure(
		[],
		'temporary-school',
		'Validation Error: Subdomain นี้มีในระบบแล้ว กรุณาใช้ชื่ออื่น'
	);

	assert.equal(result.message, 'Subdomain นี้มีในระบบแล้ว กรุณาใช้ชื่ออื่น');
});
