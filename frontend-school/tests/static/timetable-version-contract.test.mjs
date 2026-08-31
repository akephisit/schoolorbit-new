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
	assert.ok(
		openapi.components.schemas.UpdateTimetableEntryRequest.properties.instructorIds,
		'UpdateTimetableEntryRequest must expose instructorIds'
	);
	assert.ok(
		openapi.components.schemas.LearningGroupTeacherAssignment.required.includes('startsOn'),
		'LearningGroupTeacherAssignment must require startsOn'
	);
	assert.ok(
		openapi.components.schemas.LearningGroupTeacherAssignment.required.includes('displayName'),
		'LearningGroupTeacherAssignment must require displayName'
	);
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

	assert.equal((page.match(/getAcademicTermChangeSet\(/g) ?? []).length, 1);
	assert.match(page, /version\.changeSetId/);
	assert.match(
		page,
		/{:else if selectedVersion\?\.status === 'published'}[\s\S]*<AcademicChangeSetDialog[\s\S]*purpose="timetable_revision"/
	);
	assert.match(page, /<AcademicChangeReadiness/);
	assert.match(page, /draftRevision\s*\+=\s*1/);
	assert.match(page, /timetableVersionId:\s*selectedVersion\.id/);
	assert.match(page, /instructorIds:\s*formInstructorIds/);
	assert.doesNotMatch(page, /cloneTimetableVersion/);
});

test('date-based personal timetable reads carry the selected date', async () => {
	const api = await readProjectFile('src/lib/api/timetable.ts');
	const parents = await readProjectFile('src/lib/api/parents.ts');

	assert.match(api, /date:\s*requiredDate/);
	assert.match(parents, /date:\s*requiredDate/);
});

test('timetable board consumes one generated workspace and typed placement preview', async () => {
	const openapi = JSON.parse(await readProjectFile('../contracts/openapi/school-api.json'));
	const api = await readProjectFile('src/lib/api/timetable.ts');
	const boardState = await readProjectFile('src/lib/academic/timetable/board-state.ts');
	const controller = await readProjectFile(
		'src/lib/academic/timetable/workspace-controller.svelte.ts'
	);

	assert.equal(
		openapi.paths['/api/academic/timetable/workspace'].get.operationId,
		'getTimetableWorkspace'
	);
	assert.equal(
		openapi.paths['/api/academic/timetable/placement-preview'].post.operationId,
		'previewTimetablePlacement'
	);
	assert.deepEqual(
		openapi.components.schemas.TimetablePlacementSource.oneOf.map((variant) =>
			Object.keys(variant.properties).sort()
		),
		[
			['entryId', 'kind', 'rowVersion'],
			['kind', 'learningGroupId', 'learningOfferingId']
		]
	);
	assert.match(api, /TimetableWorkspace\s*=\s*Schemas\['TimetableWorkspace'\]/);
	assert.match(api, /TimetablePlacementPreview\s*=\s*Schemas\['TimetablePlacementPreview'\]/);
	assert.match(api, /operations\['getTimetableWorkspace'\]/);
	assert.match(api, /export const getTimetableWorkspace/);
	assert.match(api, /export const previewTimetablePlacement/);
	assert.doesNotMatch(api, /interface\s+TimetableWorkspace|interface\s+TimetablePlacementPreview/);
	assert.match(boardState, /entry\.instructors/);
	assert.match(boardState, /entry\.learningGroupId/);
	assert.doesNotMatch(boardState, /groupTeachers|learningGroupTeachers/);
	assert.match(controller, /\$state\.raw/);
	assert.match(controller, /\$derived/);
});
