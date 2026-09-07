import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const academicRoutes = 'src/routes/(app)/staff/academic';

async function readPage(relativePath) {
	return readFile(path.join(projectRoot, academicRoutes, relativePath, '+page.svelte'), 'utf8');
}

function assertCancellable(page, label) {
	assert.match(page, /LatestRequest/, `${label} must own the latest request`);
	assert.match(page, /isAbortError/, `${label} must ignore abort failures`);
	assert.match(page, /\.begin\(\)/, `${label} must begin a cancellable revision`);
	assert.match(page, /\.isCurrent\(revision\)/, `${label} must reject stale responses`);
	assert.match(page, /\.abort\(\)/, `${label} must abort during cleanup`);
}

test('student-year workspace uses one year relationship collection', async () => {
	const page = await readPage('student-years');
	assertCancellable(page, 'student-years');
	assert.match(page, /loadStudentYearCollections/);
	assert.match(page, /listPlacementsForAcademicYear/);
	assert.match(page, /listStudyProgramOptionsForAcademicYear/);
	assert.doesNotMatch(page, /listHomeroomPlacements\(record\.id\)/);
	assert.doesNotMatch(page, /listStudyProgramOptionsForYear/);
});

test('homeroom workspace uses one advisor relationship collection', async () => {
	const page = await readPage('homerooms');
	assertCancellable(page, 'homerooms');
	assert.match(page, /loadHomeroomCollections/);
	assert.match(page, /listHomeroomAdvisorsForAcademicYear/);
	assert.match(page, /listStudyProgramOptionsForAcademicYear/);
	assert.doesNotMatch(page, /listHomeroomAdvisors\(room\.id\)/);
	assert.doesNotMatch(page, /listStudyProgramOptionsForYear/);
});

test('curriculum workspace loads the complete structure once per version', async () => {
	const page = await readPage('curricula/[id]');
	assertCancellable(page, 'curricula');
	assert.match(page, /getCurriculumStructureWorkspace/);
	assert.doesNotMatch(page, /listProgramRequirements\(program\.id\)/);
});

test('academic core setup uses the bounded setup workspace', async () => {
	const page = await readPage('core');
	assertCancellable(page, 'academic core');
	assert.match(page, /getAcademicSetupWorkspace/);
	assert.doesNotMatch(page, /listAcademicTerms\(year\.id\)/);
	assert.doesNotMatch(page, /listBellSchedules\(year\.id\)/);
});

test('learning delivery loads one homeroom workspace without per-room requests', async () => {
	const page = await readPage('delivery');
	const table = await readFile(
		path.join(projectRoot, 'src/lib/components/learning-delivery/HomeroomDeliveryWorkspace.svelte'),
		'utf8'
	);
	assertCancellable(page, 'learning delivery');
	assert.match(page, /getHomeroomDeliveryWorkspace/);
	assert.match(page, /changeViewMode/);
	assert.doesNotMatch(table, /getLearningOffering|getLearningGroup|listLearningGroups/);
});

test('admission workspace loads study programs once for the round year', async () => {
	const page = await readPage('admission/[id]');
	assertCancellable(page, 'admission');
	assert.match(page, /listStudyProgramOptionsForAcademicYear/);
	assert.doesNotMatch(page, /listStudyProgramOptionsForYear/);
});

test('academic route consumers contain no retired study-program traversal helper', async () => {
	for (const route of ['student-years', 'homerooms', 'admission/[id]']) {
		assert.doesNotMatch(await readPage(route), /listStudyProgramOptionsForYear/, route);
	}
});

test('gradebook loads subject collections and only the selected group workspace', async () => {
	const page = await readPage('gradebook');
	assertCancellable(page, 'gradebook');
	assert.match(page, /listGradebookSubjects/);
	assert.match(page, /listLearnerEvaluationSubjects/);
	assert.match(page, /getGradebookGroupPhaseWorkspace\(\s*selectedGroupId/);
	assert.match(page, /getLearnerEvaluationWorkspace\(selectedGroupId/);
	assert.doesNotMatch(
		page,
		/Promise\.all\(\s*\w+\.map\([\s\S]{0,800}(?:getGradebookGroupPhaseWorkspace|getLearnerEvaluationWorkspace)/
	);
});

test('result preparation loads bounded readiness and only the selected detail or learner', async () => {
	const page = await readPage('results');
	assertCancellable(page, 'results');
	assert.match(page, /getAcademicResultReadiness/);
	assert.match(page, /listLearnerEvaluationSubjects/);
	assert.match(page, /getCourseResultPreparation\(selectedGroupId/);
	assert.match(page, /getActivityResultPreparation\(selectedGroupId/);
	assert.match(page, /getStudentLearnerEvaluationSummary\(studentId/);
	assert.doesNotMatch(
		page,
		/Promise\.all\(\s*(?:courseGroups|activityGroups|learnerStudents)\.map\([\s\S]{0,800}(?:getCourseResultPreparation|getActivityResultPreparation|getStudentLearnerEvaluationSummary)/
	);
});

test('result lock and correction pages use one queue or search request', async () => {
	const locks = await readPage('result-locks');
	const corrections = await readPage('result-corrections');
	assertCancellable(locks, 'result locks');
	assertCancellable(corrections, 'result corrections');
	assert.match(locks, /getAcademicResultReadiness/);
	assert.match(locks, /getLearnerEvaluationLockReadiness/);
	assert.doesNotMatch(locks, /listLearnerEvaluationSubjects/);
	assert.match(corrections, /searchEffectiveAcademicResults/);
	assert.doesNotMatch(locks, /Promise\.all\(\s*\w+\.map\(/);
	assert.doesNotMatch(corrections, /Promise\.all\(\s*\w+\.map\(/);
});
