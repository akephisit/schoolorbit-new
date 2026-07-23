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

test('generated course planning contract owns all operations and wire DTOs', async () => {
	const contract = JSON.parse(await readRepoFile('contracts/openapi/school-api.json'));
	const generated = await readRepoFile('frontend-school/src/lib/api/generated/school-api.ts');
	const academicApi = await readRepoFile('frontend-school/src/lib/api/academic.ts');
	const planningPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/planning/+page.svelte'
	);
	const expected = [
		['/api/academic/planning/courses', 'get', 'listClassroomCourses'],
		['/api/academic/planning/courses', 'post', 'assignCourses'],
		['/api/academic/planning/courses/{id}', 'put', 'updateClassroomCourse'],
		['/api/academic/planning/courses/{id}', 'delete', 'removeClassroomCourse'],
		['/api/academic/planning/courses/instructors/batch', 'post', 'batchListCourseInstructors'],
		['/api/academic/planning/courses/instructors', 'get', 'batchListCourseInstructorsFromQuery'],
		['/api/academic/planning/courses/{id}/instructors', 'get', 'listCourseInstructors'],
		['/api/academic/planning/courses/{id}/instructors', 'post', 'addCourseInstructor'],
		['/api/academic/planning/courses/{id}/instructors/{uid}', 'put', 'updateCourseInstructorRole'],
		['/api/academic/planning/courses/{id}/instructors/{uid}', 'delete', 'removeCourseInstructor'],
		[
			'/api/academic/planning/classrooms/{classroom_id}/activities',
			'get',
			'listClassroomActivities'
		],
		[
			'/api/academic/planning/classrooms/{classroom_id}/activities/{slot_id}',
			'delete',
			'removeClassroomFromActivitySlot'
		]
	];

	for (const [route, method, operationId] of expected) {
		assert.equal(contract.paths?.[route]?.[method]?.operationId, operationId, `${method} ${route}`);
		assert.match(generated, new RegExp(`\\b${operationId}:\\s*\\{`));
	}

	for (const [alias, schema] of [
		['ClassroomCourse', 'ClassroomCourse'],
		['ClassroomCourseSettings', 'ClassroomCourseSettings'],
		['CourseInstructor', 'CourseInstructor'],
		['CourseInstructorRole', 'CourseInstructorRole'],
		['AssignCoursesRequest', 'AssignCoursesRequest'],
		['UpdateCourseRequest', 'UpdateCourseRequest'],
		['AddCourseInstructorRequest', 'AddCourseInstructorRequest'],
		['BatchListCourseInstructorsRequest', 'BatchListCourseInstructorsRequest'],
		['UpdateCourseInstructorRoleRequest', 'UpdateCourseInstructorRoleRequest'],
		['ClassroomActivity', 'ClassroomActivity'],
		['CourseAssignedCountData', 'CourseAssignedCountData']
	]) {
		assert.doesNotMatch(academicApi, new RegExp(`export\\s+interface\\s+${alias}\\b`));
		assert.match(
			academicApi,
			new RegExp(`export\\s+type\\s+${alias}\\s*=\\s*Schemas\\['${schema}'\\]`)
		);
	}

	assert.match(
		academicApi,
		/listClassroomCourses = async \(\s*filters: ClassroomCourseFilters = \{\}/
	);
	assert.doesNotMatch(academicApi, /listClassroomCourses = async \([\s\S]*?param2\?: string/);
	assert.match(academicApi, /assignCourses = async \(data: AssignCoursesRequest\)/);
	assert.match(academicApi, /updateCourse = async \(\s*id: string,\s*data: UpdateCourseRequest/);
	assert.match(
		academicApi,
		/const data: BatchListCourseInstructorsRequest = \{ course_ids: courseIds \}/
	);
	assert.match(
		academicApi,
		/const data: AddCourseInstructorRequest = \{ instructor_id: instructorId, role \}/
	);
	assert.match(academicApi, /const data: UpdateCourseInstructorRoleRequest = \{ role \}/);
	assert.match(academicApi, /batchListCourseInstructorsFromQuery = async/);
	assert.match(
		planningPage,
		/listClassroomCourses\(\{\s*classroomId: selectedClassroomId,\s*semesterId: selectedTermId\s*\}\)/
	);
});
