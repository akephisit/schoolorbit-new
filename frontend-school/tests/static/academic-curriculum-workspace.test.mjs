import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = join(dirname(fileURLToPath(import.meta.url)), '../..');

async function readProjectFile(relativePath) {
	return readFile(join(projectRoot, relativePath), 'utf8');
}

test('curriculum workspace clients use generated contracts', async () => {
	const api = await readProjectFile('src/lib/api/academic-core.ts');
	const openapi = JSON.parse(await readProjectFile('../contracts/openapi/school-api.json'));

	assert.match(api, /getCurriculumOverview/);
	assert.match(api, /getCurriculumCreateOptions/);
	assert.match(api, /getCurriculumManagementOptions/);
	assert.match(api, /operations\['getCurriculumOverview'\]/);
	assert.doesNotMatch(api, /ApiResponse<unknown>|Record<string, unknown>| as Curriculum/);
	assert.ok(openapi.paths['/api/academic/curriculum-versions/{curriculumVersionId}/structure']);
	assert.ok(openapi.paths['/api/academic/curriculum-versions/{curriculumVersionId}/term-slots']);
	assert.ok(openapi.paths['/api/academic/study-programs/{studyProgramId}/structure']);
	assert.equal(openapi.paths['/api/academic/study-programs/{id}/requirements'], undefined);
	const input = openapi.components.schemas.CurriculumStructureRequirementInput.properties;
	assert.ok(input.termSlotId);
	assert.equal(input.credit, undefined);
	assert.equal(input.hours, undefined);
	assert.equal(input.recommendedTermCode, undefined);
});

test('curriculum overview is read-first and uses labeled grade selection', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/curricula/+page.svelte');
	const table = await readProjectFile(
		'src/lib/components/academic-core/CurriculumOverviewTable.svelte'
	).catch(() => '');

	assert.match(page, /getCurriculumOverview/);
	assert.match(page, /canManageAcademicCurriculum/);
	assert.doesNotMatch(page, /getCurriculumCreateOptions\([\s\S]*onMount/);
	assert.doesNotMatch(page, /gradeLevelIds:\s*''/);
	assert.doesNotMatch(page, /รหัสระดับชั้น/);
	assert.match(table, /startAcademicYearName/);
	assert.match(table, /studyProgramCount/);
});

test('curriculum detail is deep-linked and uses labeled management options', async () => {
	const meta = await readProjectFile(
		'src/routes/(app)/staff/academic/curricula/[id]/+page.ts'
	).catch(() => '');
	const page = await readProjectFile(
		'src/routes/(app)/staff/academic/curricula/[id]/+page.svelte'
	).catch(() => '');
	const editor = await readProjectFile(
		'src/lib/components/academic-core/CurriculumStructureEditor.svelte'
	);
	const comparison = await readProjectFile(
		'src/lib/components/academic-core/CurriculumProgramComparison.svelte'
	);
	const documentView = await readProjectFile(
		'src/lib/components/academic-core/CurriculumTermDocument.svelte'
	);
	const versionPanel = await readProjectFile(
		'src/lib/components/academic-core/CurriculumVersionPanel.svelte'
	);
	const createDialog = await readProjectFile(
		'src/lib/components/academic-core/CurriculumCreateDialog.svelte'
	);

	assert.match(meta, /_meta\s*=\s*\{[\s\S]*access:/);
	assert.doesNotMatch(meta, /menu:/);
	assert.match(page, /getCurriculumStructureWorkspace/);
	assert.match(page, /getCurriculumManagementOptions/);
	assert.doesNotMatch(page, /listAcademicYears/);
	assert.match(page, /CurriculumVersionView/);
	assert.match(versionPanel, /startAcademicYearName/);
	assert.match(versionPanel, /endAcademicYearName/);
	assert.match(editor, /selectedCatalogIds/);
	assert.match(editor, /CurriculumTermSlotEditor/);
	assert.match(editor, /ย้อนกลับ/);
	assert.doesNotMatch(editor, /recommendedTermCode|credit:\s|hours:\s/);
	assert.match(comparison, /ภาพรวมทุกแผนการเรียน/);
	assert.match(documentView, /โครงสร้างหลักสูตรสถานศึกษา/);
	assert.match(createDialog, /ownerOptions/);
	assert.match(createDialog, /owningOrganizationUnitId:\s*selectedOwner\.organizationUnitId/);
	assert.doesNotMatch(createDialog, /owningOrganizationUnitId:\s*null/);
});
