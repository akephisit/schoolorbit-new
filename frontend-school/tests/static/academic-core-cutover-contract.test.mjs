import assert from 'node:assert/strict';
import { access, readFile } from 'node:fs/promises';
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
	['/api/academic/setup/workspace', 'get', 'getAcademicSetupWorkspace'],
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
	['/api/academic/catalog/subjects/overview', 'get', 'getCatalogSubjectOverview'],
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
	['/api/academic/catalog/activities/overview', 'get', 'getCatalogActivityOverview'],
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
	[
		'/api/academic/curriculum-versions/{curriculumVersionId}/structure',
		'get',
		'getCurriculumStructureWorkspace'
	],
	[
		'/api/academic/curriculum-versions/{curriculumVersionId}/term-slots',
		'put',
		'replaceCurriculumTermSlots'
	],
	['/api/academic/study-program-options', 'get', 'listStudyProgramOptionsForAcademicYear'],
	['/api/academic/curriculum-versions/{id}/programs', 'get', 'listStudyPrograms'],
	['/api/academic/curriculum-versions/{id}/programs', 'post', 'createStudyProgram'],
	['/api/academic/study-programs/{id}', 'get', 'getStudyProgram'],
	['/api/academic/study-programs/{id}', 'patch', 'updateStudyProgram'],
	['/api/academic/study-programs/{studyProgramId}/structure', 'put', 'replaceCurriculumStructure'],
	['/api/academic/homerooms', 'get', 'listHomerooms'],
	['/api/academic/homerooms', 'post', 'createHomeroom'],
	['/api/academic/homerooms/{id}', 'get', 'getHomeroom'],
	['/api/academic/homerooms/{id}', 'patch', 'updateHomeroom'],
	['/api/academic/homerooms/{id}/advisors', 'get', 'listHomeroomAdvisors'],
	['/api/academic/homerooms/{id}/advisors', 'put', 'replaceHomeroomAdvisors'],
	['/api/academic/homeroom-advisors', 'get', 'listHomeroomAdvisorsForAcademicYear'],
	['/api/academic/student-years', 'get', 'listStudentAcademicYears'],
	['/api/academic/student-years', 'post', 'createStudentAcademicYear'],
	['/api/academic/student-years/candidates', 'get', 'listStudentYearCandidates'],
	['/api/academic/student-years/{id}', 'get', 'getStudentAcademicYear'],
	['/api/academic/student-years/{id}', 'patch', 'updateStudentAcademicYear'],
	['/api/academic/student-years/{id}/placements', 'get', 'listHomeroomPlacements'],
	['/api/academic/student-years/{id}/placements', 'post', 'createHomeroomPlacement'],
	['/api/academic/placements', 'get', 'listPlacementsForAcademicYear'],
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
	['/api/academic/learning-groups', 'get', 'listLearningGroupsForTerm'],
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

test('catalog overview schemas are generated from the backend contract', async () => {
	const { contract, generated } = await readContractArtifacts();
	const schemas = contract.components.schemas;

	for (const schemaName of [
		'CatalogSubjectOverview',
		'CatalogActivityOverview',
		'CatalogOwnerOption',
		'CatalogDisplayState'
	]) {
		assert.ok(schemas?.[schemaName], `${schemaName} must exist in OpenAPI`);
		assert.match(generated, new RegExp(`\\b${schemaName}:\\s*`));
	}
	for (const overviewName of ['CatalogSubjectOverview', 'CatalogActivityOverview']) {
		assert.ok(schemas[overviewName].required.includes('ownerOptions'));
	}
	for (const itemName of ['CatalogSubjectOverviewItem', 'CatalogActivityOverviewItem']) {
		assert.ok(schemas[itemName].required.includes('canManage'));
	}
});

test('scoped academic collection reads require an explicit context identifier', async () => {
	const { contract } = await readContractArtifacts();
	const scopedReads = [
		['/api/academic/terms', 'academicYearId'],
		['/api/academic/bell-schedules', 'academicYearId'],
		['/api/academic/homerooms', 'academicYearId'],
		['/api/academic/student-years', 'academicYearId'],
		['/api/academic/offerings', 'academicTermId'],
		['/api/academic/learning-groups', 'academicTermId'],
		['/api/academic/placements', 'academicYearId'],
		['/api/academic/homeroom-advisors', 'academicYearId'],
		['/api/academic/study-program-options', 'academicYearId']
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
		['CreateActivityVersionRequest', 'hoursPerTerm'],
		['UpdateActivityVersionRequest', 'hoursPerWeek'],
		['UpdateActivityVersionRequest', 'hoursPerTerm'],
		['ActivityVersion', 'hoursPerWeek'],
		['ActivityVersion', 'hoursPerTerm'],
		['CatalogCurriculumMetrics', 'credit'],
		['CatalogCurriculumMetrics', 'totalHours'],
		['CourseGradingPolicy', 'totalScore'],
		['CourseGradingPolicy', 'passingScore'],
		['ActivityAttendanceRequirement', 'minimumPercent'],
		['CourseOfferingSnapshot', 'credit'],
		['CourseOfferingSnapshot', 'hours'],
		['ActivityOfferingSnapshot', 'hours'],
		['CurriculumPreparationProposal', 'credit'],
		['CurriculumPreparationProposal', 'hours'],
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

	const courseSnapshot = schemas.CourseOfferingSnapshot;
	for (const propertyName of ['standardPeriodsPerWeek', 'weeklyPeriodTarget']) {
		assert.ok(courseSnapshot.required.includes(propertyName));
		assert.equal(courseSnapshot.properties[propertyName]?.type, 'integer');
		assert.equal(courseSnapshot.properties[propertyName]?.format, 'int32');
	}

	const updateOffering = schemas.UpdateLearningOfferingRequest;
	assert.ok(!updateOffering.required.includes('weeklyPeriodTarget'));
	assert.deepEqual(updateOffering.properties.weeklyPeriodTarget?.type, ['integer', 'null']);
	assert.equal(updateOffering.properties.weeklyPeriodTarget?.format, 'int32');

	const homeroomItem = schemas.HomeroomDeliveryItem;
	for (const propertyName of ['standardPeriodsPerWeek', 'weeklyPeriodTarget']) {
		assert.ok(!homeroomItem.required.includes(propertyName));
		assert.deepEqual(homeroomItem.properties[propertyName]?.type, ['integer', 'null']);
		assert.equal(homeroomItem.properties[propertyName]?.format, 'int32');
	}
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

test('placement transfers require an auditable reason', async () => {
	const { contract, generated } = await readContractArtifacts();
	const schema = contract.components.schemas?.TransferHomeroomPlacementRequest;

	assert.ok(schema?.required?.includes('reason'));
	assert.equal(schema?.properties?.reason?.type, 'string');
	assert.match(generated, /TransferHomeroomPlacementRequest:[\s\S]*?reason:\s*string/);
});

test('new academic workspaces own context and permission metadata', async () => {
	const routes = [
		['core', 'none', 'ACADEMIC_YEAR'],
		['catalog/subject-groups', 'none', 'ACADEMIC_CATALOG'],
		['catalog/subjects', 'none', 'ACADEMIC_CATALOG'],
		['catalog/activities', 'none', 'ACADEMIC_CATALOG'],
		['curricula', 'none', 'ACADEMIC_CURRICULUM'],
		['homerooms', 'year_required', 'HOMEROOM'],
		['student-years', 'year_required', 'STUDENT_ACADEMIC_YEAR'],
		['delivery', 'term_required', 'LEARNING_OFFERING']
	];

	for (const [route, context, permission] of routes) {
		const source = await readFile(
			path.join(repoRoot, `frontend-school/src/routes/(app)/staff/academic/${route}/+page.ts`),
			'utf8'
		);
		assert.match(source, new RegExp(`academicContext:\\s*['"]${context}['"]`), route);
		assert.match(source, new RegExp(`PERMISSION_MODULES\\.${permission}\\b`), route);
	}
});

test('new workspaces use focused typed APIs without legacy academic paths', async () => {
	const requiredFiles = [
		'frontend-school/src/lib/api/academic-core.ts',
		'frontend-school/src/lib/api/learning-delivery.ts',
		'frontend-school/src/lib/components/academic-core/AcademicYearTermEditor.svelte',
		'frontend-school/src/lib/components/academic-core/CatalogVersionHistory.svelte',
		'frontend-school/src/lib/components/academic-core/CurriculumStructureEditor.svelte',
		'frontend-school/src/lib/components/academic-core/CurriculumTermDocument.svelte',
		'frontend-school/src/lib/components/academic-core/HomeroomEditor.svelte',
		'frontend-school/src/lib/components/academic-core/StudentYearPlacementEditor.svelte',
		'frontend-school/src/lib/components/academic-core/StudentYearTransferDialog.svelte',
		'frontend-school/src/lib/components/learning-delivery/OfferingCreateDialog.svelte',
		'frontend-school/src/lib/components/learning-delivery/LearningGroupEditor.svelte',
		'frontend-school/src/lib/components/learning-delivery/RosterPreviewPanel.svelte',
		'frontend-school/src/lib/components/learning-delivery/OfferingCurriculumPreview.svelte'
	];

	for (const file of requiredFiles) await access(path.join(repoRoot, file));

	const wrappers = `${await readFile(path.join(repoRoot, requiredFiles[0]), 'utf8')}\n${await readFile(path.join(repoRoot, requiredFiles[1]), 'utf8')}`;
	assert.match(wrappers, /generated\/school-api/);
	assert.doesNotMatch(
		wrappers,
		/\bunknown\b|Record<string,\s*unknown>|\/api\/academic\/(structure|semesters|classrooms|enrollments|planning\/courses|subjects|study-plans)/
	);

	const allUi = [];
	for (const file of requiredFiles.slice(2))
		allUi.push(await readFile(path.join(repoRoot, file), 'utf8'));
	assert.doesNotMatch(allUi.join('\n'), /numberOfTerms|number_of_terms|GPA|เกรดเฉลี่ย/);

	const coreEditor = allUi[0];
	assert.match(coreEditor, /onUpdateYear/);
	assert.match(coreEditor, /onUpdateTerm/);
	assert.match(coreEditor, /onReplaceBellSchedulePeriods/);
	assert.match(coreEditor, /BellSchedulePeriodsStep/);
});

test('legacy academic workspace routes are removed without aliases', async () => {
	const retired = [
		'frontend-school/src/routes/(app)/staff/academic/structure/+page.ts',
		'frontend-school/src/routes/(app)/staff/academic/structure/+page.svelte',
		'frontend-school/src/routes/(app)/staff/academic/subjects/+page.ts',
		'frontend-school/src/routes/(app)/staff/academic/subjects/+page.svelte',
		'frontend-school/src/routes/(app)/staff/academic/study-plans/+page.ts',
		'frontend-school/src/routes/(app)/staff/academic/study-plans/+page.svelte',
		'frontend-school/src/routes/(app)/staff/academic/classrooms/+page.ts',
		'frontend-school/src/routes/(app)/staff/academic/classrooms/+page.svelte',
		'frontend-school/src/routes/(app)/staff/academic/enrollments/+page.ts',
		'frontend-school/src/routes/(app)/staff/academic/enrollments/+page.svelte',
		'frontend-school/src/routes/(app)/staff/academic/planning/+page.ts',
		'frontend-school/src/routes/(app)/staff/academic/planning/+page.svelte',
		'frontend-school/src/routes/(app)/staff/academic/activities/+page.ts',
		'frontend-school/src/routes/(app)/staff/academic/activities/+page.svelte',
		'frontend-school/src/routes/(app)/staff/academic/activities/[id]/+page.svelte',
		'frontend-school/src/routes/(app)/staff/academic/subject-groups/+page.ts',
		'frontend-school/src/routes/(app)/staff/academic/subject-groups/+page.svelte',
		'frontend-school/src/routes/(app)/staff/academic/subject-groups/[id]/+page.svelte'
	];

	for (const file of retired) {
		await assert.rejects(access(path.join(repoRoot, file)), undefined, file);
	}
});

test('generated contract contains no retired academic routes or legacy fields', async () => {
	const { contract, generated } = await readContractArtifacts();
	const academicPrefix = '/api/academic/';
	const retiredPaths = [
		'structure',
		'semesters',
		'classrooms',
		'enrollments',
		'planning/courses',
		'subjects',
		'study-plans'
	].map((suffix) => `${academicPrefix}${suffix}`);

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
		for (const retiredProperty of [
			['semester', 'Id'].join(''),
			['classroom', 'CourseId'].join(''),
			'isActive'
		]) {
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
