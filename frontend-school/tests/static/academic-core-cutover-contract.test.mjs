import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../../..');

async function readContractArtifacts() {
	const [contractSource, generated] = await Promise.all([
		readFile(path.join(repoRoot, 'contracts/openapi/school-api.json'), 'utf8'),
		readFile(path.join(repoRoot, 'frontend-school/src/lib/api/generated/school-api.ts'), 'utf8')
	]);

	return { contract: JSON.parse(contractSource), generated };
}

const operations = [
	['/api/academic/context/options', 'get', 'listAcademicContextOptions'],
	['/api/academic/years', 'get', 'listAcademicYears'],
	['/api/academic/years', 'post', 'createAcademicYear'],
	['/api/academic/years/{id}', 'get', 'getAcademicYear'],
	['/api/academic/years/{id}', 'patch', 'updateAcademicYear'],
	['/api/academic/terms', 'get', 'listAcademicTerms'],
	['/api/academic/terms', 'post', 'createAcademicTerm'],
	['/api/academic/terms/{id}', 'get', 'getAcademicTerm'],
	['/api/academic/terms/{id}', 'patch', 'updateAcademicTerm'],
	['/api/academic/terms/{id}', 'delete', 'deleteAcademicTerm'],
	['/api/academic/bell-schedules', 'get', 'listBellSchedules'],
	['/api/academic/bell-schedules', 'post', 'createBellSchedule'],
	['/api/academic/bell-schedules/{id}', 'get', 'getBellSchedule'],
	['/api/academic/bell-schedules/{id}', 'patch', 'updateBellSchedule'],
	['/api/academic/bell-schedules/{id}/periods', 'get', 'listBellSchedulePeriods'],
	['/api/academic/bell-schedules/{id}/periods', 'put', 'replaceBellSchedulePeriods'],
	['/api/academic/grade-progressions', 'get', 'listGradeProgressions'],
	['/api/academic/grade-progressions', 'put', 'replaceGradeProgressions'],
	['/api/academic/catalog/subjects', 'get', 'listCatalogSubjects'],
	['/api/academic/catalog/subjects', 'post', 'createCatalogSubject'],
	['/api/academic/catalog/subjects/{id}', 'get', 'getCatalogSubject'],
	['/api/academic/catalog/subjects/{id}', 'patch', 'updateCatalogSubject'],
	['/api/academic/catalog/subjects/{id}/versions', 'get', 'listSubjectVersions'],
	['/api/academic/catalog/subjects/{id}/versions', 'post', 'createSubjectVersion'],
	['/api/academic/catalog/subject-versions/{id}', 'get', 'getSubjectVersion'],
	['/api/academic/catalog/subject-versions/{id}', 'patch', 'updateSubjectVersion'],
	['/api/academic/catalog/subject-versions/{id}/publish', 'post', 'publishSubjectVersion'],
	['/api/academic/catalog/subjects/{id}/default-teachers', 'get', 'listSubjectDefaultTeachers'],
	['/api/academic/catalog/subjects/{id}/default-teachers', 'put', 'replaceSubjectDefaultTeachers'],
	['/api/academic/catalog/subject-groups', 'get', 'listSubjectGroups'],
	['/api/academic/catalog/subject-groups', 'post', 'createSubjectGroup'],
	['/api/academic/catalog/subject-groups/{id}', 'get', 'getSubjectGroup'],
	['/api/academic/catalog/subject-groups/{id}', 'patch', 'updateSubjectGroup'],
	['/api/academic/catalog/subject-groups/{id}', 'delete', 'deleteSubjectGroup'],
	['/api/academic/catalog/activities', 'get', 'listCatalogActivities'],
	['/api/academic/catalog/activities', 'post', 'createCatalogActivity'],
	['/api/academic/catalog/activities/{id}', 'get', 'getCatalogActivity'],
	['/api/academic/catalog/activities/{id}', 'patch', 'updateCatalogActivity'],
	['/api/academic/catalog/activities/{id}/versions', 'get', 'listActivityVersions'],
	['/api/academic/catalog/activities/{id}/versions', 'post', 'createActivityVersion'],
	['/api/academic/catalog/activity-versions/{id}', 'get', 'getActivityVersion'],
	['/api/academic/catalog/activity-versions/{id}', 'patch', 'updateActivityVersion'],
	['/api/academic/catalog/activity-versions/{id}/publish', 'post', 'publishActivityVersion'],
	['/api/academic/catalog/activities/{id}/default-teachers', 'get', 'listActivityDefaultTeachers'],
	[
		'/api/academic/catalog/activities/{id}/default-teachers',
		'put',
		'replaceActivityDefaultTeachers'
	],
	['/api/academic/curricula', 'get', 'listCurricula'],
	['/api/academic/curricula', 'post', 'createCurriculum'],
	['/api/academic/curricula/{id}', 'get', 'getCurriculum'],
	['/api/academic/curricula/{id}', 'patch', 'updateCurriculum'],
	['/api/academic/curricula/{id}/versions', 'get', 'listCurriculumVersions'],
	['/api/academic/curricula/{id}/versions', 'post', 'createCurriculumVersion'],
	['/api/academic/curriculum-versions/{id}', 'get', 'getCurriculumVersion'],
	['/api/academic/curriculum-versions/{id}', 'patch', 'updateCurriculumVersion'],
	['/api/academic/curriculum-versions/{id}/publish', 'post', 'publishCurriculumVersion'],
	['/api/academic/curriculum-versions/{id}/programs', 'get', 'listStudyPrograms'],
	['/api/academic/curriculum-versions/{id}/programs', 'post', 'createStudyProgram'],
	['/api/academic/study-programs/{id}', 'get', 'getStudyProgram'],
	['/api/academic/study-programs/{id}', 'patch', 'updateStudyProgram'],
	['/api/academic/study-programs/{id}/requirements', 'get', 'listProgramRequirements'],
	['/api/academic/study-programs/{id}/requirements', 'put', 'replaceProgramRequirements'],
	['/api/academic/homerooms', 'get', 'listHomerooms'],
	['/api/academic/homerooms', 'post', 'createHomeroom'],
	['/api/academic/homerooms/{id}', 'get', 'getHomeroom'],
	['/api/academic/homerooms/{id}', 'patch', 'updateHomeroom'],
	['/api/academic/homerooms/{id}/advisors', 'get', 'listHomeroomAdvisors'],
	['/api/academic/homerooms/{id}/advisors', 'put', 'replaceHomeroomAdvisors'],
	['/api/academic/student-years', 'get', 'listStudentAcademicYears'],
	['/api/academic/student-years', 'post', 'createStudentAcademicYear'],
	['/api/academic/student-years/{id}', 'get', 'getStudentAcademicYear'],
	['/api/academic/student-years/{id}', 'patch', 'updateStudentAcademicYear'],
	['/api/academic/student-years/{id}/placements', 'post', 'createHomeroomPlacement'],
	['/api/academic/placements/{id}/transfer', 'post', 'transferHomeroomPlacement'],
	['/api/academic/offerings', 'get', 'listLearningOfferings'],
	['/api/academic/offerings', 'post', 'createLearningOffering'],
	[
		'/api/academic/offerings/preview-from-curriculum',
		'post',
		'previewLearningOfferingsFromCurriculum'
	],
	['/api/academic/offerings/apply-from-curriculum', 'post', 'applyLearningOfferingsFromCurriculum'],
	['/api/academic/offerings/{id}', 'get', 'getLearningOffering'],
	['/api/academic/offerings/{id}', 'patch', 'updateLearningOffering'],
	['/api/academic/offerings/{id}/publish', 'post', 'publishLearningOffering'],
	['/api/academic/offerings/{id}/groups', 'get', 'listLearningGroups'],
	['/api/academic/offerings/{id}/groups', 'post', 'createLearningGroup'],
	['/api/academic/learning-groups/{id}', 'get', 'getLearningGroup'],
	['/api/academic/learning-groups/{id}', 'patch', 'updateLearningGroup'],
	['/api/academic/learning-groups/{id}/homerooms', 'get', 'listLearningGroupHomerooms'],
	['/api/academic/learning-groups/{id}/homerooms', 'put', 'replaceLearningGroupHomerooms'],
	['/api/academic/learning-groups/{id}/teachers', 'get', 'listLearningGroupTeachers'],
	['/api/academic/learning-groups/{id}/teachers', 'put', 'replaceLearningGroupTeachers'],
	['/api/academic/learning-groups/{id}/roster', 'get', 'previewLearningGroupRoster'],
	['/api/academic/learning-groups/{id}/roster', 'put', 'applyLearningGroupRoster'],
	['/api/academic/learning-groups/{id}/roster/publish', 'post', 'publishLearningGroupRoster']
];

test('generated contract owns every academic core and learning delivery operation', async () => {
	const { contract, generated } = await readContractArtifacts();

	for (const [route, method, operationId] of operations) {
		assert.equal(contract.paths?.[route]?.[method]?.operationId, operationId, `${method} ${route}`);
		assert.match(generated, new RegExp(`\\b${operationId}:\\s*\\{`));
	}

	const operationIds = Object.values(contract.paths).flatMap((pathItem) =>
		Object.values(pathItem).flatMap((operation) => operation.operationId ?? [])
	);
	assert.equal(new Set(operationIds).size, operationIds.length, 'operation IDs must be unique');
});

test('scoped academic collection reads require an explicit context identifier', async () => {
	const { contract } = await readContractArtifacts();
	const scopedReads = [
		['/api/academic/terms', 'academicYearId'],
		['/api/academic/bell-schedules', 'academicYearId'],
		['/api/academic/homerooms', 'academicYearId'],
		['/api/academic/student-years', 'academicYearId'],
		['/api/academic/offerings', 'academicTermId']
	];

	for (const [route, parameterName] of scopedReads) {
		const parameter = contract.paths?.[route]?.get?.parameters?.find(
			(candidate) => candidate.name === parameterName && candidate.in === 'query'
		);
		assert.equal(parameter?.required, true, `${route} must require ${parameterName}`);
		assert.equal(parameter?.schema?.format, 'uuid', `${route} ${parameterName} must be a UUID`);
	}
});

test('academic exact values and offering variants keep their wire semantics', async () => {
	const { contract } = await readContractArtifacts();
	const schemas = contract.components.schemas;
	const decimalFields = [
		['CreateSubjectVersionRequest', 'credit'],
		['UpdateSubjectVersionRequest', 'credit'],
		['SubjectVersion', 'credit'],
		['CreateActivityVersionRequest', 'hoursPerWeek'],
		['UpdateActivityVersionRequest', 'hoursPerWeek'],
		['ActivityVersion', 'hoursPerWeek'],
		['ProgramRequirementInput', 'credit'],
		['ProgramRequirementInput', 'hours'],
		['CourseGradingPolicy', 'totalScore'],
		['CourseGradingPolicy', 'passingScore'],
		['ActivityAttendanceRequirement', 'minimumPercent'],
		['CourseOfferingSnapshot', 'credit'],
		['CourseOfferingSnapshot', 'hours'],
		['ActivityOfferingSnapshot', 'hours'],
		['CurriculumOfferingPreviewItem', 'credit'],
		['CurriculumOfferingPreviewItem', 'hours'],
		['ActivityResult', 'attendancePercent']
	];

	for (const [schemaName, propertyName] of decimalFields) {
		const property = schemas?.[schemaName]?.properties?.[propertyName];
		const types = Array.isArray(property?.type) ? property.type : [property?.type];
		assert.ok(types.includes('string'), `${schemaName}.${propertyName} must be a string`);
		assert.ok(!types.includes('number'), `${schemaName}.${propertyName} must not be a number`);
	}

	const createOffering = schemas.CreateLearningOfferingRequest;
	assert.equal(createOffering?.oneOf?.length, 2);
	const variantTags = createOffering.oneOf.map((variant) => {
		const tagSchema = variant.allOf?.find((part) => part.properties?.kind);
		assert.ok(tagSchema?.required?.includes('kind'));
		return tagSchema.properties.kind.enum?.[0];
	});
	assert.deepEqual(variantTags.sort(), ['activity', 'course']);
});

test('academic JSON operations use response envelopes', async () => {
	const { contract } = await readContractArtifacts();

	for (const [route, method] of operations) {
		const operation = contract.paths[route][method];
		for (const [status, response] of Object.entries(operation.responses)) {
			const schema = response.content?.['application/json']?.schema;
			if (!schema) continue;
			const reference = schema.$ref ?? '';
			if (status.startsWith('2')) {
				assert.match(
					reference,
					/^#\/components\/schemas\/ApiResponse_/,
					`${method} ${route} ${status}`
				);
			} else {
				assert.match(
					reference,
					/^#\/components\/schemas\/ApiErrorResponse/,
					`${method} ${route} ${status}`
				);
			}
		}
	}
});

test('generated contract contains no retired academic routes or legacy fields', async () => {
	const { contract, generated } = await readContractArtifacts();
	const retiredPaths = [
		'/api/academic/structure',
		'/api/academic/semesters',
		'/api/academic/classrooms',
		'/api/academic/enrollments',
		'/api/academic/planning/courses',
		'/api/academic/subjects',
		'/api/academic/study-plans'
	];

	for (const retiredPath of retiredPaths) {
		assert.equal(contract.paths?.[retiredPath], undefined, `${retiredPath} must stay removed`);
	}

	for (const schemaName of [
		'AcademicYear',
		'AcademicYearOption',
		'AcademicTerm',
		'AcademicTermOption',
		'CreateAcademicYearRequest',
		'UpdateAcademicYearRequest',
		'CreateAcademicTermRequest',
		'UpdateAcademicTermRequest'
	]) {
		const properties = schemasOrEmpty(contract, schemaName);
		for (const retiredProperty of ['semesterId', 'classroomCourseId', 'isActive']) {
			assert.equal(
				properties[retiredProperty],
				undefined,
				`${schemaName}.${retiredProperty} must stay removed`
			);
		}
	}

	for (const retiredOperation of [
		'getAcademicStructure',
		'createSemester',
		'listClassrooms',
		'enrollStudents',
		'listPlannedCourses'
	]) {
		assert.doesNotMatch(generated, new RegExp(`\\b${retiredOperation}:\\s*\\{`));
	}
});

function schemasOrEmpty(contract, schemaName) {
	const schema = contract.components.schemas?.[schemaName];
	assert.ok(schema, `missing schema ${schemaName}`);
	return schema.properties ?? {};
}
