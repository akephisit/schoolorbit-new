import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../../..');

function readRepoFile(relativePath) {
	return readFile(path.join(repoRoot, relativePath), 'utf8');
}

test('generated activity template contract owns all batch operations and wire DTOs', async () => {
	const contract = JSON.parse(await readRepoFile('contracts/openapi/school-api.json'));
	const generated = await readRepoFile('frontend-school/src/lib/api/generated/school-api.ts');
	const academicApi = await readRepoFile('frontend-school/src/lib/api/academic.ts');
	const expected = [
		[
			'/api/academic/study-plan-versions/{id}/activities',
			'get',
			'listStudyPlanActivities'
		],
		[
			'/api/academic/study-plan-versions/{id}/activities',
			'post',
			'addStudyPlanActivity'
		],
		['/api/academic/study-plan-activities/{id}', 'put', 'updateStudyPlanActivity'],
		['/api/academic/study-plan-activities/{id}', 'delete', 'deleteStudyPlanActivity'],
		[
			'/api/academic/activities/generate-from-plan',
			'post',
			'generateActivitiesFromStudyPlan'
		],
		['/api/academic/activity-catalog', 'get', 'listActivityCatalog'],
		['/api/academic/activity-catalog', 'post', 'createActivityCatalog'],
		['/api/academic/activity-catalog/{id}', 'put', 'updateActivityCatalog'],
		['/api/academic/activity-catalog/{id}', 'delete', 'deleteActivityCatalog'],
		[
			'/api/academic/activity-catalog/{id}/default-instructors',
			'get',
			'listActivityCatalogDefaultInstructors'
		],
		[
			'/api/academic/activity-catalog/{id}/default-instructors',
			'post',
			'addActivityCatalogDefaultInstructor'
		],
		[
			'/api/academic/activity-catalog/{id}/default-instructors/{uid}',
			'delete',
			'removeActivityCatalogDefaultInstructor'
		],
		[
			'/api/academic/activity-catalog/{id}/default-instructors/{uid}',
			'put',
			'updateActivityCatalogDefaultInstructorRole'
		]
	];

	for (const [route, method, operationId] of expected) {
		assert.equal(contract.paths?.[route]?.[method]?.operationId, operationId, `${method} ${route}`);
		assert.match(generated, new RegExp(`\\b${operationId}:\\s*\\{`));
	}

	const operationIds = Object.values(contract.paths).flatMap((pathItem) =>
		Object.values(pathItem).flatMap((operation) => operation.operationId ?? [])
	);
	assert.equal(operationIds.length, 165);
	assert.equal(new Set(operationIds).size, 165);

	for (const [alias, schema] of [
		['ActivitySlot', 'ActivitySlot'],
		['ActivityGroup', 'ActivityGroup'],
		['SlotInstructor', 'SlotInstructorInfo'],
		['SlotClassroomAssignment', 'SlotClassroomAssignment'],
		['StudyPlanVersionActivity', 'StudyPlanVersionActivity'],
		['CreatePlanActivityRequest', 'CreatePlanActivityRequest'],
		['UpdatePlanActivityRequest', 'UpdatePlanActivityRequest'],
		['GenerateActivitiesFromPlanRequest', 'GenerateActivitiesFromPlanRequest'],
		['GenerateActivitiesFromPlanResponse', 'GenerateActivitiesFromPlanOutcome'],
		['ActivityCatalog', 'ActivityCatalog'],
		['ActivityCatalogType', 'ActivityCatalogType'],
		['ActivitySchedulingMode', 'ActivitySchedulingMode'],
		['CatalogDefaultInstructor', 'CatalogDefaultInstructor'],
		['CatalogDefaultInstructorInput', 'CatalogDefaultInstructorInput'],
		['CreateCatalogRequest', 'CreateCatalogRequest'],
		['UpdateCatalogRequest', 'UpdateCatalogRequest'],
		['AddCatalogDefaultInstructorRequest', 'AddCatalogDefaultInstructorRequest'],
		[
			'UpdateCatalogDefaultInstructorRoleRequest',
			'UpdateCatalogDefaultInstructorRoleRequest'
		]
	]) {
		assert.doesNotMatch(academicApi, new RegExp(`export\\s+interface\\s+${alias}\\b`));
		assert.match(
			academicApi,
			new RegExp(`export\\s+type\\s+${alias}\\s*=\\s*Schemas\\['${schema}'\\]`)
		);
	}

	assert.match(
		academicApi,
		/addPlanActivity = async \(\s*versionId: string,\s*data: CreatePlanActivityRequest\s*\)/
	);
	assert.match(
		academicApi,
		/updatePlanActivity = async \(\s*id: string,\s*data: UpdatePlanActivityRequest\s*\)/
	);
	assert.match(academicApi, /createActivityCatalog = async \(data: CreateCatalogRequest\)/);
	assert.match(
		academicApi,
		/updateActivityCatalog = async \(id: string, data: UpdateCatalogRequest\)/
	);
	assert.match(
		academicApi,
		/generateActivitiesFromPlan = async \(\s*data: GenerateActivitiesFromPlanRequest\s*\)/
	);
});
