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
	const contract = JSON.parse(await readProjectFile('../contracts/openapi/school-api.json'));
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
	assert.doesNotMatch(
		api,
		/ApiResponse<unknown>|Record<string, unknown>|\sas\sAcademicMenuTemplate/
	);
});

test('menu administration previews the recommended structure before explicit apply', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/menu/+page.svelte');
	const dialog = await readProjectFile('src/lib/components/menu/AcademicMenuTemplateDialog.svelte');

	assert.match(page, /PERMISSIONS\.MENU_READ_ALL/);
	assert.match(page, /PERMISSIONS\.MENU_UPDATE_ALL/);
	assert.match(page, /AcademicMenuTemplateDialog/);
	assert.match(page, /canApply=\{canUpdateMenu\}/);
	assert.match(dialog, /ใช้โครงสร้างงานวิชาการแนะนำ/);
	assert.match(dialog, /previewRecommendedAcademicMenuTemplate/);
	assert.match(dialog, /applyRecommendedAcademicMenuTemplate/);
	assert.match(dialog, /preview\.revision/);
	assert.match(dialog, /sections_to_create|sectionsToCreate/);
	assert.match(dialog, /untouched_custom_item_count|untouchedCustomItemCount/);
	assert.match(
		dialog,
		/if \(error instanceof ApiClientError && error\.status === 409\)[\s\S]*await loadPreview\(\);[\s\S]*errorMessage = 'ข้อมูลเมนูเปลี่ยนแล้ว กรุณาตรวจสอบรายการอีกครั้ง'/
	);
	assert.doesNotMatch(dialog, /onMount/);
});
