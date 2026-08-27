import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = join(dirname(fileURLToPath(import.meta.url)), '../..');

async function readProjectFile(relativePath) {
	return readFile(join(projectRoot, relativePath), 'utf8');
}

test('academic menu template publishes generated preview and apply contracts', async () => {
	const api = await readProjectFile('src/lib/api/menu-admin.ts');
	const contract = JSON.parse(
		await readProjectFile('../contracts/openapi/school-api.json')
	);
	const previewPath = contract.paths['/api/admin/menu/templates/academic/recommended'];
	const applyPath = contract.paths['/api/admin/menu/templates/academic/recommended/apply'];

	assert.equal(previewPath?.get?.operationId, 'previewRecommendedAcademicMenuTemplate');
	assert.equal(applyPath?.post?.operationId, 'applyRecommendedAcademicMenuTemplate');
	for (const schema of [
		'AcademicMenuTemplatePreview',
		'AcademicMenuTemplateMove',
		'AcademicMenuTemplateSection',
		'AcademicMenuTemplateApplyResult',
		'ApplyAcademicMenuTemplateRequest'
	]) {
		assert.ok(contract.components.schemas[schema], `missing ${schema}`);
	}
	assert.match(api, /AcademicMenuTemplatePreview/);
	assert.match(api, /previewRecommendedAcademicMenuTemplate/);
	assert.match(api, /applyRecommendedAcademicMenuTemplate/);
	assert.doesNotMatch(api, /ApiResponse<unknown>|Record<string, unknown>|\sas\sAcademicMenuTemplate/);
});
