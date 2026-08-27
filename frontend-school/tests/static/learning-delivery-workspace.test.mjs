import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');

const readProjectFile = (relativePath) => readFile(path.join(projectRoot, relativePath), 'utf8');

test('delivery workspace uses generated term query contracts', async () => {
	const api = await readProjectFile('src/lib/api/learning-delivery.ts');
	assert.match(api, /operations\['getLearningDeliveryOverview'\]/);
	assert.match(api, /operations\['getLearningDeliveryManagementOptions'\]/);
	assert.match(api, /getLearningDeliveryOverview/);
	assert.match(api, /getLearningDeliveryManagementOptions/);
	assert.match(api, /getLearningOffering/);
	assert.match(api, /getLearningGroup/);
	assert.doesNotMatch(api, /academic_term_id|ApiResponse<unknown>|Record<string, unknown>/);
});

test('delivery overview is term scoped, filterable, and keeps management options lazy', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/delivery/+page.svelte');
	const table = await readProjectFile(
		'src/lib/components/learning-delivery/OfferingOverviewTable.svelte'
	);
	const dialog = await readProjectFile(
		'src/lib/components/learning-delivery/OfferingCreateDialog.svelte'
	);
	assert.match(page, /getLearningDeliveryOverview/);
	assert.match(page, /academicTermId/);
	assert.match(page, /kind=activity|kindFilter|initialKind/);
	assert.doesNotMatch(page, /getLearningDeliveryManagementOptions\([\s\S]*onMount/);
	assert.doesNotMatch(dialog, /catalogVersionId[^\n]*<Input/);
	assert.doesNotMatch(dialog, /gradeLevelId[^\n]*<Input/);
	assert.match(table, /groupsWithoutPrimaryTeacher/);
	assert.match(table, /publishedRosterCount/);
});
