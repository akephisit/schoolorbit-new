import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const readProjectFile = (relativePath) => readFile(path.join(projectRoot, relativePath), 'utf8');

test('generated timetable editing operations require an explicit version', async () => {
	const openapi = JSON.parse(await readProjectFile('../contracts/openapi/school-api.json'));

	for (const [route, method, operationId] of [
		['/api/academic/timetable-versions', 'get', 'listTimetableVersions'],
		['/api/academic/timetable-versions/resolve', 'get', 'resolveTimetableVersion'],
		['/api/academic/timetable-versions/{source_id}/clone', 'post', 'cloneTimetableVersion']
	]) {
		assert.equal(openapi.paths[route][method].operationId, operationId);
	}

	const listParameters = openapi.paths['/api/academic/timetable'].get.parameters;
	const versionParameter = listParameters.find(
		(parameter) => parameter.in === 'query' && parameter.name === 'timetableVersionId'
	);
	assert.equal(versionParameter?.required, true);

	for (const schemaName of [
		'CreateTimetableEntryRequest',
		'CreateBatchTimetableEntriesRequest',
		'UpdateTimetableEntryRequest',
		'SwapTimetableEntriesRequest',
		'ValidateMovesRequest',
		'FromCurrentRequest',
		'ApplyTemplateRequest',
		'ClearTimetableRequest'
	]) {
		assert.ok(
			openapi.components.schemas[schemaName].required.includes('timetableVersionId'),
			`${schemaName} must require timetableVersionId`
		);
	}
});

test('typed timetable wrapper owns version list resolve clone and version-scoped queries', async () => {
	const api = await readProjectFile('src/lib/api/timetable.ts');

	assert.match(api, /type TimetableVersion = Schemas\['TimetableVersion'\]/);
	assert.match(api, /operations\['listTimetableVersions'\]/);
	assert.match(api, /operations\['resolveTimetableVersion'\]/);
	assert.match(api, /operations\['cloneTimetableVersion'\]/);
	assert.match(api, /export const listTimetableVersions/);
	assert.match(api, /export const resolveTimetableVersion/);
	assert.match(api, /export const cloneTimetableVersion/);
	assert.match(api, /timetableVersionId:\s*requiredVersion/);
	assert.doesNotMatch(api, /ApiResponse<unknown>|Record<string,\s*unknown>|\bas any\b/);
});

test('academic timetable selects one URL-backed version and disables published editing', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/timetable/+page.svelte');

	assert.equal((page.match(/listTimetableVersions\(/g) ?? []).length, 1);
	assert.match(page, /timetableVersionId/);
	assert.match(page, /selectedVersion\?\.status === 'draft'/);
	assert.match(page, /replaceState/);
	assert.match(page, /current.*upcoming.*draft/is);
	assert.match(page, /เผยแพร่แล้ว/);
	assert.match(page, /อ่านอย่างเดียว/);
	assert.match(page, /timetableVersionId:\s*selectedVersion\.id/);
});

test('timetable revision creation reuses the date-effective academic change workflow', async () => {
	const dialog = await readProjectFile(
		'src/lib/components/learning-delivery/AcademicChangeSetDialog.svelte'
	);

	assert.match(dialog, /'operational_change'\s*\|\s*'timetable_revision'/);
	assert.match(dialog, /purpose\s*=\s*'operational_change'/);
	assert.match(dialog, /สร้างรุ่นตารางสอนใหม่/);
	assert.match(dialog, /วันที่เริ่มใช้รุ่นใหม่/);
	assert.match(dialog, /createAcademicTermChangeSet/);
	assert.doesNotMatch(dialog, /cloneTimetableVersion/);
});

test('timetable workspace links one change set to the selected version and invalidates readiness after edits', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/timetable/+page.svelte');

	assert.equal((page.match(/listAcademicTermChangeSets\(/g) ?? []).length, 1);
	assert.match(page, /targetTimetableVersionId\s*===\s*selectedVersion\?\.id/);
	assert.match(page, /<AcademicChangeSetDialog[\s\S]*purpose="timetable_revision"/);
	assert.match(page, /<AcademicChangeReadiness/);
	assert.match(page, /draftRevision\s*\+=\s*1/);
	assert.match(page, /timetableVersionId:\s*selectedVersion\.id/);
	assert.match(page, /instructorIds:\s*\[\]/);
	assert.doesNotMatch(page, /instructorIds:[^\n]*teacherAssignments\.map/);
	assert.doesNotMatch(page, /cloneTimetableVersion/);
});

test('date-based personal timetable reads carry the selected date', async () => {
	const api = await readProjectFile('src/lib/api/timetable.ts');
	const parents = await readProjectFile('src/lib/api/parents.ts');

	assert.match(api, /date:\s*requiredDate/);
	assert.match(parents, /date:\s*requiredDate/);
});
