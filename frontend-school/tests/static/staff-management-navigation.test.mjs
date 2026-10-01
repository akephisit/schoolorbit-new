import assert from 'node:assert/strict';
import test from 'node:test';

test('staff links carry list context and reject external or unrelated return destinations', async () => {
	const { staffReturnHref, withStaffReturn } =
		await import('../../src/lib/navigation/staff-management.ts');
	assert.equal(
		staffReturnHref(
			new URL(
				'https://school.test/staff/manage/one?returnTo=%2Fstaff%2Fmanage%3Fsearch%3Dteacher%26page%3D2%26status%3Dinactive'
			)
		),
		'/staff/manage?search=teacher&page=2&status=inactive'
	);
	for (const target of [
		'https://evil.test/staff/manage',
		'//evil.test/staff/manage',
		'/staff/roles',
		'/staff/manage/../roles',
		'/staff/manage?returnTo=https://evil.test'
	]) {
		assert.equal(
			staffReturnHref(
				new URL(`https://school.test/staff/manage/one?returnTo=${encodeURIComponent(target)}`)
			),
			'/staff/manage'
		);
	}
	assert.equal(
		withStaffReturn('/staff/manage/one/edit', '/staff/manage?search=teacher&page=2'),
		'/staff/manage/one/edit?returnTo=%2Fstaff%2Fmanage%3Fsearch%3Dteacher%26page%3D2'
	);
});
