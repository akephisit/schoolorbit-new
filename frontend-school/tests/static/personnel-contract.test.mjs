import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
test('personnel patch retains omitted fields and explicitly clears references', async () => {
	const {
		buildStaffPersonnelPatch,
		ACADEMIC_RANK_LABELS,
		EDUCATION_LEVEL_LABELS,
		personnelDrilldownHref
	} = await import('../../src/lib/forms/staff-personnel.ts');
	assert.deepEqual(buildStaffPersonnelPatch({ major: 'คณิตศาสตร์' }, { major: null }), {
		major: null
	});
	assert.equal(
		buildStaffPersonnelPatch({ academic_rank: 'none' }, { academic_rank: 'none' }),
		undefined
	);
	assert.equal(ACADEMIC_RANK_LABELS.none, 'ไม่มีวิทยฐานะ');
	assert.equal(EDUCATION_LEVEL_LABELS.bachelor, 'ปริญญาตรี');
	const href = personnelDrilldownHref(
		'subject_group',
		{ key: '55000000-0000-4000-8000-000000000001', label: 'Math', count: 2 },
		'active'
	);
	assert.equal(
		new URL(href, 'https://school.test').searchParams.get('subject_group_id'),
		'55000000-0000-4000-8000-000000000001'
	);
	const { staffReturnHref } = await import('../../src/lib/navigation/staff-management.ts');
	assert.equal(
		staffReturnHref(
			new URL(`https://school.test/staff/manage/one?returnTo=${encodeURIComponent(href)}`)
		),
		href
	);
});
test('personnel endpoints use generated canonical types without legacy input text', async () => {
	const contract = await readFile(
		new URL('../../src/lib/api/generated/school-api.ts', import.meta.url),
		'utf8'
	);
	for (const name of [
		'getPersonnelOverview',
		'listStaffJobPositions',
		'UpdateStaffInfoRequest',
		'StaffAcademicRank'
	])
		assert.ok(contract.includes(name), name);
});

test('canonical personnel contract uses education text and a read-only position catalog', async () => {
	const contract = JSON.parse(
		await readFile(new URL('../../../contracts/openapi/school-api.json', import.meta.url), 'utf8')
	);
	for (const dto of ['CreateStaffInfoRequest', 'UpdateStaffInfoRequest']) {
		const fields = contract.components.schemas[dto].properties;
		assert.equal(Object.hasOwn(fields, 'major_id'), false);
		assert.equal(Object.hasOwn(fields, 'university_id'), false);
		assert.ok(fields.major && fields.university && fields.job_position_id);
	}
	const fields = contract.components.schemas.StaffInfoResponse.properties;
	for (const key of ['major', 'university'])
		assert.ok(JSON.stringify(fields[key]).includes('string'));
	assert.ok(JSON.stringify(fields.job_position).includes('StaffJobPositionSummary'));
	assert.deepEqual(Object.keys(contract.paths['/api/staff/job-positions']), ['get']);
	assert.equal(contract.paths['/api/staff/reference-items'], undefined);
	assert.equal(contract.paths['/api/staff/reference-items/{id}'], undefined);
});

test('education text normalization uses scalar counts and retains spelling', async () => {
	const { normalizeStaffEducationText, buildStaffPersonnelPatch } =
		await import('../../src/lib/forms/staff-personnel.ts');
	assert.equal(normalizeStaffEducationText('  คณิตศาสตร์  ประยุกต์  '), 'คณิตศาสตร์  ประยุกต์');
	assert.equal(normalizeStaffEducationText('   '), null);
	for (const value of ['ก'.repeat(200), '😀'.repeat(200)])
		assert.equal(normalizeStaffEducationText(value), value);
	for (const value of ['ก'.repeat(201), '😀'.repeat(201), 'text\n', 'text\t', 'text\u0085'])
		assert.throws(() => normalizeStaffEducationText(value));
	assert.equal(
		buildStaffPersonnelPatch({ university: 'สถาบันทดสอบ' }, { university: 'สถาบันทดสอบ' }),
		undefined
	);
	assert.deepEqual(
		buildStaffPersonnelPatch(
			{ major: 'คณิตศาสตร์', academic_rank: 'none' },
			{ major: '   ', academic_rank: 'none' }
		),
		{ major: null }
	);
});
