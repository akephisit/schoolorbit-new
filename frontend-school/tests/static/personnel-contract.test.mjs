import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import { staffPersonnelDraft } from '../../src/lib/forms/staff-personnel.ts';
const personnel = (fields = {}) => ({ ...staffPersonnelDraft(null), ...fields });
test('personnel patch retains omitted fields and explicitly clears references', async () => {
	const {
		buildStaffPersonnelPatch,
		ACADEMIC_RANK_LABELS,
		EDUCATION_LEVEL_LABELS,
		personnelDrilldownHref
	} = await import('../../src/lib/forms/staff-personnel.ts');
	assert.deepEqual(
		buildStaffPersonnelPatch(
			personnel({ teaching_license_number: 'synthetic' }),
			personnel({ teaching_license_number: '' })
		),
		{
			teaching_license_number: null
		}
	);
	assert.equal(buildStaffPersonnelPatch(personnel(), personnel()), undefined);
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
		assert.ok(fields.career);
		assert.ok(dto === 'CreateStaffInfoRequest' ? fields.educations : fields.education);
		assert.equal(fields.education_level, undefined);
		assert.equal(Object.hasOwn(fields, 'job_position_id'), false);
		assert.equal(Object.hasOwn(fields, 'academic_rank'), false);
	}
	const fields = contract.components.schemas.StaffInfoResponse.properties;
	for (const key of ['major', 'university'])
		assert.ok(
			JSON.stringify(contract.components.schemas.StaffEducation.properties[key]).includes('string')
		);
	assert.ok(fields.educations && fields.education_revision && fields.highest_education_level);
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
		buildStaffPersonnelPatch(
			personnel({
				educations: [
					{
						key: 'new',
						id: null,
						education_level: 'bachelor',
						major: null,
						university: 'สถาบันทดสอบ'
					}
				]
			}),
			personnel({
				educations: [
					{
						key: 'new',
						id: null,
						education_level: 'bachelor',
						major: null,
						university: 'สถาบันทดสอบ'
					}
				]
			})
		),
		undefined
	);
	assert.deepEqual(
		buildStaffPersonnelPatch(
			personnel({
				educations: [
					{
						key: 'a',
						id: 'saved',
						education_level: 'bachelor',
						major: 'คณิตศาสตร์',
						university: null
					}
				],
				education_revision: 3
			}),
			personnel({ educations: [] })
		),
		{ education: { expected_revision: 3, items: [] } }
	);
});

test('clearing a date picker sends an explicit licence expiry clear', async () => {
	const { buildStaffPersonnelPatch } = await import('../../src/lib/forms/staff-personnel.ts');
	assert.deepEqual(
		buildStaffPersonnelPatch(
			personnel({ teaching_license_expiry: '2030-01-01' }),
			personnel({ teaching_license_expiry: undefined })
		),
		{ teaching_license_expiry: null }
	);
});

test('licence replacement preserves expected reference and explicit removal', async () => {
	const { buildStaffPersonnelPatch } = await import('../../src/lib/forms/staff-personnel.ts');
	const before = personnel({ teaching_license_file_id: 'saved-file' });
	assert.deepEqual(
		buildStaffPersonnelPatch(before, personnel({ teaching_license_file_id: 'new-file' })),
		{ teaching_license_file: { expected_file_id: 'saved-file', file_id: 'new-file' } }
	);
	assert.deepEqual(buildStaffPersonnelPatch(before, personnel()), {
		teaching_license_file: { expected_file_id: 'saved-file', file_id: null }
	});
});
