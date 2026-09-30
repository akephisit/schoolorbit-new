import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
const source = (path) => readFile(new URL(`../../src/${path}`, import.meta.url), 'utf8');
test('facility route owns default buildings while the inactive rooms tab stays lazy', async () => {
	const loader = await source('routes/(app)/staff/facility/buildings/+page.ts');
	assert.match(loader, /listBuildings\(\{ requestFetch: fetch \}\)/);
	assert.match(loader, /school:app-identity/);
	assert.doesNotMatch(loader, /listRooms/);
	const page = await source('routes/(app)/staff/facility/buildings/+page.svelte');
	assert.doesNotMatch(page, /onMount/);
	assert.match(page, /activeTab === 'rooms'/);
	assert.match(page, /roomsRequest/);
	assert.match(page, /\$effect\.pre/);
});
test('facility uses generated resource and operation contracts', async () => {
	const api = await source('lib/api/facility.ts');
	assert.match(api, /components\['schemas'\]/);
	assert.match(api, /operations\['listFacilityRooms'\]/);
	assert.doesNotMatch(api, /export interface (Building|Room)\b/);
});

test('facility primary and mutation endpoints have generated OpenAPI operations', async () => {
	const contract = JSON.parse(
		await readFile(new URL('../../../contracts/openapi/school-api.json', import.meta.url), 'utf8')
	);
	for (const [resource, entity] of [
		['buildings', 'Building'],
		['rooms', 'Room']
	]) {
		for (const [method, operation, suffix] of [
			['get', `listFacility${entity}s`, ''],
			['post', `createFacility${entity}`, ''],
			['put', `updateFacility${entity}`, '/{id}'],
			['delete', `deleteFacility${entity}`, '/{id}']
		]) {
			assert.equal(
				contract.paths[`/api/facilities/${resource}${suffix}`]?.[method]?.operationId,
				operation
			);
		}
	}
});
