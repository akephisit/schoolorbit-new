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
	assert.deepEqual(buildStaffPersonnelPatch({ major_id: 'major' }, { major_id: null }), {
		major_id: null
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
		'listStaffReferenceItems',
		'createStaffReferenceItem',
		'updateStaffReferenceItem',
		'UpdateStaffInfoRequest',
		'StaffAcademicRank'
	])
		assert.ok(contract.includes(name), name);
});

test('canonical personnel contract excludes legacy text inputs and exposes reference summaries', async () => {
	const contract = JSON.parse(
		await readFile(new URL('../../../contracts/openapi/school-api.json', import.meta.url), 'utf8')
	);
	for (const dto of ['CreateStaffInfoRequest', 'UpdateStaffInfoRequest']) {
		const fields = contract.components.schemas[dto].properties;
		assert.equal(Object.hasOwn(fields, 'major'), false);
		assert.equal(Object.hasOwn(fields, 'university'), false);
		assert.ok(fields.major_id && fields.university_id && fields.job_position_id);
	}
	const fields = contract.components.schemas.StaffInfoResponse.properties;
	for (const key of ['major', 'university', 'job_position'])
		assert.ok(JSON.stringify(fields[key]).includes('StaffReferenceSummary'));
});
