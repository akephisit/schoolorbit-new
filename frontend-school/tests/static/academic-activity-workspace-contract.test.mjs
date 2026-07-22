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

test('generated activity workspace contract owns all wire DTOs and operations', async () => {
	const contract = JSON.parse(await readRepoFile('contracts/openapi/school-api.json'));
	const generated = await readRepoFile('frontend-school/src/lib/api/generated/school-api.ts');
	const academicApi = await readRepoFile('frontend-school/src/lib/api/academic.ts');
	const expectedOperationIds = [
		'listActivitySlots',
		'updateActivitySlot',
		'deleteActivitySlot',
		'listActivitySlotInstructors',
		'addActivitySlotInstructor',
		'addActivitySlotInstructorsBatch',
		'removeActivitySlotInstructor',
		'removeAllActivitySlotInstructors',
		'deleteAllActivitySlotGroups',
		'deleteActivitySlotTimetableEntries',
		'listActivitySlotClassroomAssignments',
		'upsertActivitySlotClassroomAssignments',
		'deleteAllActivitySlotClassroomAssignments',
		'deleteActivitySlotClassroomAssignment',
		'listActivityGroups',
		'createActivityGroup',
		'updateActivityGroup',
		'deleteActivityGroup',
		'listActivityGroupMembers',
		'addActivityGroupMembers',
		'removeActivityGroupMember',
		'updateActivityGroupMemberResult',
		'listActivityGroupInstructors',
		'addActivityGroupInstructor',
		'removeActivityGroupInstructor',
		'listMyActivityEnrollments',
		'selfEnrollActivityGroup',
		'selfUnenrollActivityGroup'
	];
	const contractOperationIds = Object.values(contract.paths).flatMap((pathItem) =>
		Object.values(pathItem).flatMap((operation) => operation.operationId ?? [])
	);

	assert.equal(contractOperationIds.length, 177);
	assert.equal(new Set(contractOperationIds).size, 177);
	for (const operationId of expectedOperationIds) {
		assert.ok(contractOperationIds.includes(operationId), operationId);
		assert.match(generated, new RegExp(`\\b${operationId}:\\s*\\{`));
	}

	for (const [alias, schema] of [
		['ActivitySlotFilter', 'ActivitySlotFilter'],
		['UpdateActivitySlotRequest', 'UpdateActivitySlotRequest'],
		['ActivityRegistrationType', 'ActivityRegistrationType'],
		['AddSlotInstructorRequest', 'AddSlotInstructorRequest'],
		['AddSlotInstructorsBatchRequest', 'AddSlotInstructorsBatchRequest'],
		['ActivityGroupFilter', 'ActivityGroupFilter'],
		['CreateActivityGroupRequest', 'CreateActivityGroupRequest'],
		['UpdateActivityGroupRequest', 'UpdateActivityGroupRequest'],
		['ActivityGroupMember', 'ActivityGroupMember'],
		['ActivityMemberResult', 'ActivityMemberResult'],
		['AddMembersRequest', 'AddMembersRequest'],
		['UpdateMemberResultRequest', 'UpdateMemberResultRequest'],
		['ActivityInstructor', 'InstructorInfo'],
		['ActivityGroupInstructorRole', 'ActivityGroupInstructorRole'],
		['InstructorRoleRequest', 'InstructorRoleRequest'],
		['UpsertSlotClassroomAssignmentRequest', 'UpsertSlotClassroomAssignmentRequest'],
		['BatchUpsertSlotClassroomAssignmentsRequest', 'BatchUpsertSlotClassroomAssignmentsRequest'],
		['ActivityInsertedCountData', 'ActivityInsertedCountData'],
		['ActivityAddedCountData', 'ActivityAddedCountData'],
		['ActivityDeletedCountData', 'ActivityDeletedCountData'],
		['ActivityProcessedCountData', 'ActivityProcessedCountData']
	]) {
		assert.doesNotMatch(academicApi, new RegExp(`export\\s+interface\\s+${alias}\\b`));
		assert.match(
			academicApi,
			new RegExp(`export\\s+type\\s+${alias}\\s*=\\s*Schemas\\['${schema}'\\]`)
		);
	}

	assert.match(academicApi, /listActivitySlots = async \(\s*filter: ActivitySlotFilter/);
	assert.match(
		academicApi,
		/updateActivitySlot = async \(\s*id: string,\s*data: UpdateActivitySlotRequest/
	);
	assert.match(academicApi, /createActivityGroup = async \(data: CreateActivityGroupRequest\)/);
	assert.match(
		academicApi,
		/updateActivityGroup = async \(id: string, data: UpdateActivityGroupRequest\)/
	);
});
