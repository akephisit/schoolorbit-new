import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
const root = path.resolve(import.meta.dirname, '../..');
const read = (name) => readFile(path.join(root, name), 'utf8');
test('canonical curriculum contracts retire nested editions and owner inputs', async () => {
	const api = await read('src/lib/api/academic-core.ts');
	const document = JSON.parse(await read('../contracts/openapi/school-api.json'));
	assert.match(api, /Schemas\['CopyStudyProgramRequest'\]/);
	for (const endpoint of [
		'/api/academic/curricula/{id}/levels',
		'/api/academic/curricula/{id}/publish',
		'/api/academic/curriculum-levels/{id}/structure',
		'/api/academic/curriculum-levels/{id}/copy-program'
	])
		assert.ok(document.paths[endpoint], endpoint);
	assert.ok(!Object.keys(document.paths).some((p) => p.includes('curriculum-versions')));
	assert.doesNotMatch(
		api,
		/cloneCurriculumVersion|listCurriculumVersions|ApiResponse<unknown>| as Curriculum/
	);
	for (const name of [
		'CreateCurriculumRequest',
		'CreateCurriculumLevelRequest',
		'CreateStudyProgramRequest'
	]) {
		const fields = document.components.schemas[name].properties;
		for (const field of [
			'code',
			'nameEn',
			'owningOrganizationUnitId',
			'startAcademicYearId',
			'endAcademicYearId'
		])
			assert.equal(fields[field], undefined);
	}
	const fields = document.components.schemas.CurriculumStructureRequirementInput.properties;
	assert.ok(fields.termSlotId);
	assert.equal(fields.credit, undefined);
	assert.equal(fields.hours, undefined);
});
test('editions and level structures have separate route-owned reads and management gates', async () => {
	const main = await read('src/routes/(app)/staff/academic/curricula/+page.svelte');
	const edition = await read('src/routes/(app)/staff/academic/curricula/[id]/+page.svelte');
	const level = await read(
		'src/routes/(app)/staff/academic/curricula/[id]/levels/[levelId]/+page.svelte'
	);
	const loader = await read(
		'src/routes/(app)/staff/academic/curricula/[id]/levels/[levelId]/+page.ts'
	);
	assert.match(main, /data\.overview/);
	assert.doesNotMatch(main, /groupCurriculaByRevision/);
	assert.match(edition, /CurriculumLevelCreateDialog/);
	assert.match(edition, /publishCurriculum/);
	assert.match(loader, /workspace\.level\.editionId !== params\.id/);
	assert.match(level, /data\.structure/);
	assert.match(level, /getCurriculumManagementOptions/);
	assert.match(level, /CurriculumProgramCopyDialog/);
	assert.match(level, /ACADEMIC_CURRICULUM_MANAGE_SCHOOL/);
	assert.match(level, /level\.status === 'draft'/);
	for (const page of [main, edition, level])
		assert.doesNotMatch(page, /\bonMount\s*\(|\binvalidateAll\s*\(/);
});
test('curriculum forms use names and grade choices; copying and room selection preserve edition context', async () => {
	for (const file of [
		'CurriculumCreateDialog',
		'CurriculumLevelCreateDialog',
		'CurriculumProgramCreateDialog',
		'CurriculumStructureEditor'
	]) {
		const form = await read(`src/lib/components/academic-core/${file}.svelte`);
		assert.doesNotMatch(
			form,
			/owningOrganizationUnitId|ownerOptions|programDraft\.code|nameEn:|credit:\s|hours:\s/
		);
	}
	const copy = await read('src/lib/components/academic-core/CurriculumProgramCopyDialog.svelte');
	assert.match(copy, /sourceRowVersion/);
	assert.match(copy, /destinationRowVersion/);
	assert.match(copy, /status === 'published'/);
	const room = await read('src/lib/components/academic-core/HomeroomEditor.svelte');
	assert.match(room, /homeroom-edition/);
	assert.match(room, /program\.editionId === editionId/);
	assert.match(room, /programsForGrade/);
});
