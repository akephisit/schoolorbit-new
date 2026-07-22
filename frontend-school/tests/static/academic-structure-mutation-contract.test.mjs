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

test('generated academic structure contract owns all batch operations and DTOs', async () => {
	const contract = JSON.parse(await readRepoFile('contracts/openapi/school-api.json'));
	const generated = await readRepoFile('frontend-school/src/lib/api/generated/school-api.ts');
	const academicApi = await readRepoFile('frontend-school/src/lib/api/academic.ts');
	const expected = [
		['/api/academic/structure', 'get', 'getAcademicStructure'],
		['/api/academic/levels', 'post', 'createGradeLevel'],
		['/api/academic/levels/{id}', 'delete', 'deleteGradeLevel'],
		['/api/academic/years', 'post', 'createAcademicYear'],
		['/api/academic/years/{id}', 'put', 'updateAcademicYear'],
		['/api/academic/years/{id}/active', 'put', 'setActiveAcademicYear'],
		['/api/academic/years/{id}/levels', 'get', 'getAcademicYearLevels'],
		['/api/academic/years/{id}/levels', 'put', 'updateAcademicYearLevels'],
		['/api/academic/semesters', 'post', 'createSemester'],
		['/api/academic/semesters/{id}', 'put', 'updateSemester'],
		['/api/academic/semesters/{id}', 'delete', 'deleteSemester'],
		['/api/academic/classrooms', 'get', 'listClassrooms'],
		['/api/academic/classrooms', 'post', 'createClassroom'],
		['/api/academic/classrooms/{id}', 'put', 'updateClassroom'],
		['/api/academic/enrollments', 'post', 'enrollStudents'],
		['/api/academic/enrollments/class/{id}', 'get', 'listClassEnrollments'],
		['/api/academic/enrollments/{id}', 'delete', 'removeEnrollment'],
		['/api/academic/enrollments/{id}/number', 'put', 'updateEnrollmentNumber'],
		['/api/academic/enrollments/class/{id}/auto-number', 'post', 'autoAssignClassNumbers']
	];

	for (const [route, method, operationId] of expected) {
		assert.equal(contract.paths?.[route]?.[method]?.operationId, operationId, `${method} ${route}`);
		assert.match(generated, new RegExp(`\\b${operationId}:\\s*\\{`));
	}

	const operationIds = Object.values(contract.paths).flatMap((pathItem) =>
		Object.values(pathItem).flatMap((operation) => operation.operationId ?? [])
	);
	assert.equal(operationIds.length, 184);
	assert.equal(new Set(operationIds).size, 184);

	for (const [alias, schema] of [
		['AcademicYear', 'AcademicYear'],
		['Semester', 'Semester'],
		['GradeLevel', 'GradeLevelResponse'],
		['AcademicStructureData', 'AcademicStructure'],
		['ClassroomAdvisor', 'ClassroomAdvisor'],
		['Classroom', 'Classroom'],
		['StudentEnrollment', 'StudentEnrollment'],
		['CreateAcademicYearRequest', 'CreateAcademicYearRequest'],
		['UpdateAcademicYearRequest', 'UpdateAcademicYearRequest'],
		['CreateSemesterRequest', 'CreateSemesterRequest'],
		['UpdateSemesterRequest', 'UpdateSemesterRequest'],
		['CreateGradeLevelRequest', 'CreateGradeLevelRequest'],
		['CreateClassroomRequest', 'CreateClassroomRequest'],
		['UpdateClassroomRequest', 'UpdateClassroomRequest'],
		['EnrollStudentRequest', 'EnrollStudentRequest']
	]) {
		assert.doesNotMatch(academicApi, new RegExp(`export\\s+interface\\s+${alias}\\b`));
		assert.match(
			academicApi,
			new RegExp(`export\\s+type\\s+${alias}\\s*=\\s*Schemas\\['${schema}'\\]`)
		);
	}

	assert.match(academicApi, /type EmptyResponseData = Schemas\['EmptyData'\]/);
	assert.match(academicApi, /createAcademicYear = async \(data: CreateAcademicYearRequest\)/);
	assert.match(
		academicApi,
		/updateAcademicYear = async \(id: string, data: UpdateAcademicYearRequest\)/
	);
	assert.match(academicApi, /createClassroom = async \(data: CreateClassroomRequest\)/);
	assert.match(academicApi, /enrollStudents = async \(data: EnrollStudentRequest\)/);
});
