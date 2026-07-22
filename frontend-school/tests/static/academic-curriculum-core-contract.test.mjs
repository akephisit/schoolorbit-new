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

test('generated curriculum core contract owns all batch operations and wire DTOs', async () => {
	const contract = JSON.parse(await readRepoFile('contracts/openapi/school-api.json'));
	const generated = await readRepoFile('frontend-school/src/lib/api/generated/school-api.ts');
	const academicApi = await readRepoFile('frontend-school/src/lib/api/academic.ts');
	const expected = [
		['/api/academic/subjects/groups', 'get', 'listSubjectGroups'],
		[
			'/api/academic/subjects/default-instructors',
			'get',
			'batchListSubjectDefaultInstructors'
		],
		['/api/academic/subjects', 'get', 'listSubjects'],
		['/api/academic/subjects', 'post', 'createSubject'],
		['/api/academic/subjects/{id}', 'put', 'updateSubject'],
		['/api/academic/subjects/{id}', 'delete', 'deleteSubject'],
		[
			'/api/academic/subjects/{id}/default-instructors',
			'get',
			'listSubjectDefaultInstructors'
		],
		[
			'/api/academic/subjects/{id}/default-instructors',
			'post',
			'addSubjectDefaultInstructor'
		],
		[
			'/api/academic/subjects/{id}/default-instructors/{uid}',
			'delete',
			'removeSubjectDefaultInstructor'
		],
		[
			'/api/academic/subjects/{id}/default-instructors/{uid}',
			'put',
			'updateSubjectDefaultInstructorRole'
		],
		['/api/academic/study-plans', 'get', 'listStudyPlans'],
		['/api/academic/study-plans', 'post', 'createStudyPlan'],
		['/api/academic/study-plans/{id}', 'get', 'getStudyPlan'],
		['/api/academic/study-plans/{id}', 'put', 'updateStudyPlan'],
		['/api/academic/study-plans/{id}', 'delete', 'deleteStudyPlan'],
		['/api/academic/study-plan-versions', 'get', 'listStudyPlanVersions'],
		['/api/academic/study-plan-versions', 'post', 'createStudyPlanVersion'],
		['/api/academic/study-plan-versions/{id}', 'get', 'getStudyPlanVersion'],
		['/api/academic/study-plan-versions/{id}', 'put', 'updateStudyPlanVersion'],
		['/api/academic/study-plan-versions/{id}', 'delete', 'deleteStudyPlanVersion'],
		[
			'/api/academic/study-plan-versions/{id}/subjects',
			'get',
			'listStudyPlanSubjects'
		],
		[
			'/api/academic/study-plan-versions/{id}/subjects',
			'post',
			'addSubjectsToStudyPlanVersion'
		],
		['/api/academic/study-plan-subjects/{id}', 'delete', 'deleteStudyPlanSubject'],
		[
			'/api/academic/planning/generate-from-plan',
			'post',
			'generateCoursesFromStudyPlan'
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
		['SubjectGroup', 'SubjectGroup'],
		['Subject', 'Subject'],
		['SubjectDefaultInstructor', 'SubjectDefaultInstructor'],
		['CreateSubjectRequest', 'CreateSubjectRequest'],
		['UpdateSubjectRequest', 'UpdateSubjectRequest'],
		['StudyPlan', 'StudyPlan'],
		['CreateStudyPlanRequest', 'CreateStudyPlanRequest'],
		['UpdateStudyPlanRequest', 'UpdateStudyPlanRequest'],
		['StudyPlanVersion', 'StudyPlanVersion'],
		['CreateStudyPlanVersionRequest', 'CreateStudyPlanVersionRequest'],
		['UpdateStudyPlanVersionRequest', 'UpdateStudyPlanVersionRequest'],
		['StudyPlanSubject', 'StudyPlanSubject'],
		['SubjectInPlan', 'SubjectInPlan'],
		['GenerateCoursesFromPlanRequest', 'GenerateCoursesFromPlanRequest'],
		['GenerateCoursesFromPlanResponse', 'GenerateCoursesData']
	]) {
		assert.doesNotMatch(academicApi, new RegExp(`export\\s+interface\\s+${alias}\\b`));
		assert.match(
			academicApi,
			new RegExp(`export\\s+type\\s+${alias}\\s*=\\s*Schemas\\['${schema}'\\]`)
		);
	}

	assert.match(academicApi, /createSubject = async \(data: CreateSubjectRequest\)/);
	assert.match(academicApi, /updateSubject = async \(id: string, data: UpdateSubjectRequest\)/);
	assert.match(academicApi, /createStudyPlan = async \(data: CreateStudyPlanRequest\)/);
	assert.match(
		academicApi,
		/updateStudyPlan = async \(id: string, data: UpdateStudyPlanRequest\)/
	);
	assert.match(
		academicApi,
		/createStudyPlanVersion = async \(data: CreateStudyPlanVersionRequest\)/
	);
	assert.match(
		academicApi,
		/updateStudyPlanVersion = async \(id: string, data: UpdateStudyPlanVersionRequest\)/
	);
	assert.match(
		academicApi,
		/generateCoursesFromPlan = async \(\s*data: GenerateCoursesFromPlanRequest\s*\)/
	);
});
