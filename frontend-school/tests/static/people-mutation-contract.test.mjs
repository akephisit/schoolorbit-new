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

test('generated people mutation contract owns all migrated operations and DTOs', async () => {
	const contract = JSON.parse(await readRepoFile('contracts/openapi/school-api.json'));
	const generated = await readRepoFile('frontend-school/src/lib/api/generated/school-api.ts');
	const staffApi = await readRepoFile('frontend-school/src/lib/api/staff.ts');
	const studentsApi = await readRepoFile('frontend-school/src/lib/api/students.ts');
	const achievementApi = await readRepoFile('frontend-school/src/lib/api/achievement.ts');
	const achievementTypes = await readRepoFile('frontend-school/src/lib/types/achievement.ts');
	const expected = [
		['/api/staff', 'post', 'createStaff'],
		['/api/staff/{id}', 'put', 'updateStaff'],
		['/api/staff/{id}', 'delete', 'deleteStaff'],
		['/api/student/profile', 'put', 'updateStudentProfile'],
		['/api/students', 'post', 'createStudent'],
		['/api/students/{id}', 'put', 'updateStudent'],
		['/api/students/{id}', 'delete', 'deleteStudent'],
		['/api/students/{id}/parents', 'post', 'addStudentParent'],
		['/api/students/{id}/parents/{parent_id}', 'delete', 'removeStudentParent'],
		['/api/achievements', 'get', 'listAchievements'],
		['/api/achievements', 'post', 'createAchievement'],
		['/api/achievements/{id}', 'put', 'updateAchievement'],
		['/api/achievements/{id}', 'delete', 'deleteAchievement']
	];

	for (const [route, method, operationId] of expected) {
		assert.equal(contract.paths?.[route]?.[method]?.operationId, operationId, `${method} ${route}`);
		assert.match(generated, new RegExp(`\\b${operationId}:\\s*\\{`));
	}

	const operationIds = Object.values(contract.paths).flatMap((pathItem) =>
		Object.values(pathItem).flatMap((operation) => operation.operationId ?? [])
	);
	assert.equal(
		new Set(operationIds).size,
		operationIds.length,
		'people operations must remain part of a globally unique operation inventory'
	);

	for (const [source, names] of [
		[staffApi, ['CreateStaffRequest', 'UpdateStaffRequest']],
		[
			studentsApi,
			[
				'CreateStudentRequest',
				'UpdateStudentRequest',
				'UpdateOwnProfileRequest',
				'CreateParentRequest'
			]
		],
		[achievementTypes, ['Achievement', 'CreateAchievementRequest', 'UpdateAchievementRequest']]
	]) {
		for (const name of names) {
			assert.doesNotMatch(source, new RegExp(`export\\s+interface\\s+${name}\\b`));
			assert.match(source, new RegExp(`export\\s+type\\s+${name}\\s*=\\s*Schemas\\['${name}'\\]`));
		}
	}

	assert.match(staffApi, /createStaff[\s\S]*apiClient\.post<UuidIdData>\('\/api\/staff'/);
	assert.match(
		staffApi,
		/updateStaff[\s\S]*apiClient\.put<EmptyData>\(`\/api\/staff\/\$\{staffId\}`/
	);
	assert.match(
		staffApi,
		/deleteStaff[\s\S]*apiClient\.delete<EmptyData>\(`\/api\/staff\/\$\{staffId\}`/
	);
	assert.match(
		studentsApi,
		/createStudent[\s\S]*apiClient\.post<CreateStudentResponse>\('\/api\/students'/
	);
	assert.match(
		studentsApi,
		/updateStudent[\s\S]*apiClient\.put<EmptyData>\(`\/api\/students\/\$\{id\}`/
	);
	assert.match(
		studentsApi,
		/deleteStudent[\s\S]*apiClient\.delete<EmptyData>\(`\/api\/students\/\$\{id\}`/
	);
	assert.match(
		studentsApi,
		/updateOwnProfile[\s\S]*apiClient\.put<EmptyData>\('\/api\/student\/profile'/
	);
	assert.match(studentsApi, /addParentToStudent[\s\S]*apiClient\.post<EmptyData>/);
	assert.match(studentsApi, /removeParentFromStudent[\s\S]*apiClient\.delete<EmptyData>/);
	assert.match(achievementApi, /deleteAchievement[\s\S]*ApiResponse<EmptyData>/);
	assert.match(achievementApi, /apiClient\.delete<EmptyData>\(`\/api\/achievements\/\$\{id\}`/);
});

test('staff profile achievement mutations patch local state without refetching', async () => {
	const page = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/manage/[id]/+page.svelte'
	);

	assert.match(page, /const savedAchievement = res\.data;/);
	assert.match(page, /achievements = \[savedAchievement, \.\.\.achievements\];/);
	assert.match(
		page,
		/achievements = achievements\.map\([\s\S]{0,100}item\.id === savedAchievement\.id \? savedAchievement : item/
	);
	assert.match(page, /const deletedId = deleteId;/);
	assert.match(page, /achievements = achievements\.filter\(\(item\) => item\.id !== deletedId\);/);
	assert.equal(
		page.match(/\bloadAchievements\(\);/g)?.length,
		1,
		'only the initial profile load should fetch achievements'
	);
});
