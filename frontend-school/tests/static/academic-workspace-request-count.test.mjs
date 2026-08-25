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

test('curriculum workspace loads programs and requirements once per version', async () => {
	const page = await readPage('curricula');
	assertCancellable(page, 'curricula');
	assert.match(page, /getCurriculumProgramWorkspace/);
	assert.doesNotMatch(page, /listProgramRequirements\(program\.id\)/);
});

test('academic core setup uses the bounded setup workspace', async () => {
	const page = await readPage('core');
	assertCancellable(page, 'academic core');
	assert.match(page, /getAcademicSetupWorkspace/);
	assert.doesNotMatch(page, /listAcademicTerms\(year\.id\)/);
	assert.doesNotMatch(page, /listBellSchedules\(year\.id\)/);
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
