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
