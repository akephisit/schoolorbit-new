import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const read = (relativePath) => readFile(path.join(projectRoot, relativePath), 'utf8');

test('canonical timetable operations require explicit term and current timetable context', async () => {
	const openapi = JSON.parse(await read('../contracts/openapi/school-api.json'));
	const requiredTimetableSchemas = [
		'CreateOrdinaryTimetableBlockRequest',
		'CreateSynchronizedTimetableBlockRequest',
		'CreateStructuralTimetableBlocksRequest',
		'UpdateTimetableBlockRequest',
		'RemoveTimetableBlockTargetRequest',
		'SwapTimetableBlocksRequest',
		'FromCurrentRequest',
		'ApplyTemplateRequest',
		'ClearTimetableRequest'
	];

	assert.equal(
		openapi.paths['/api/academic/timetable-blocks/workspace'].get.operationId,
		'getTimetableBlockWorkspace'
	);
	assert.equal(
		openapi.paths['/api/academic/timetable-blocks/placement-preview'].post.operationId,
		'previewTimetableBlockPlacement'
	);
	for (const schemaName of requiredTimetableSchemas) {
		assert.ok(
			openapi.components.schemas[schemaName].required.includes('timetableId'),
			`${schemaName} must require timetableId`
		);
	}
	assert.ok(
		openapi.components.schemas.CreateOrdinaryTimetableBlockRequest.required.includes(
			'academicTermId'
		)
	);
});

test('typed timetable wrapper exposes only canonical block resources', async () => {
	const api = await read('src/lib/api/timetable.ts');

	assert.match(api, /TimetableBlock\s*=\s*Schemas\['TimetableBlock'\]/);
	assert.match(api, /TimetableBlockWorkspace\s*=\s*Schemas\['TimetableBlockWorkspace'\]/);
	assert.match(
		api,
		/TimetableBlockPlacementPreview\s*=\s*Schemas\['TimetableBlockPlacementPreview'\]/
	);
	assert.match(api, /operations\['getTimetableBlockWorkspace'\]/);
	assert.match(api, /export const getTimetableBlockWorkspace/);
	assert.match(api, /export const createOrdinaryTimetableBlock/);
	assert.match(api, /export const createSynchronizedTimetableBlock/);
	assert.match(api, /export const createStructuralTimetableBlocks/);
	assert.match(api, /export const removeTimetableBlockTarget/);
	assert.doesNotMatch(
		api,
		/TimetableEntry|WholeSchoolTimetable|\/api\/academic\/timetable\/workspace/
	);
	assert.doesNotMatch(api, /ApiResponse<unknown>|Record<string,\s*unknown>|\bas any\b/);
});

test('academic timetable resolves the current term table and keeps read mode distinct from editing', async () => {
 const page=await read('src/routes/(app)/staff/academic/timetable/+page.svelte');
 const loader=await read('src/routes/(app)/staff/academic/timetable/+page.ts');
 const header=await read('src/lib/components/academic/timetable/TimetableWorkspaceHeader.svelte');
 assert.match(loader,/getTimetable\(academicTermId/);
 assert.match(loader,/timetableId: timetable.id/);
 assert.match(page,/controller\?\.canEdit/);
 assert.match(page,/shallow:\s*true/);
 assert.match(header,/ตารางสอนปัจจุบัน/);assert.match(header,/โหมดดู/);
 assert.match(page,/timetableId:\s*controller\.workspace\.timetable\.id/);
 assert.doesNotMatch(loader+page,/listTimetableVersions|cloneTimetableVersion|publishTimetableVersion/);
});

test('direct board persistence retains every create, move and swap path with teacher export', async () => {
 const page=await read('src/routes/(app)/staff/academic/timetable/+page.svelte');
 for(const operation of ['createOrdinaryTimetableBlock','createSynchronizedTimetableBlock','updateTimetableBlock','swapTimetableBlocks'])assert.match(page,new RegExp(`await ${operation}\\(`));
 assert.match(page,/downloadTeacherLoadWorkbook/);assert.match(page,/controller\.workspace\.blocks/);
 assert.doesNotMatch(page,/cloneTimetableVersion|previewTimetablePublication|publishTimetableVersion/);
});

test('placement feedback remains independent from the accepted write and pending preview', async () => {
 const page=await read('src/routes/(app)/staff/academic/timetable/+page.svelte');
 assert.match(page,/canEdit/);assert.match(page,/previewTimetableBlockPlacement/);
 assert.match(page,/persistOptimisticPlacement/);assert.match(page,/placementFailureMessage/);
 assert.doesNotMatch(page,/controller\?\.canEdit\s*&&\s*!busy\s*&&\s*!previewing/);
});

test('block placement contract models ordinary synchronized and existing sources', async () => {
	const openapi = JSON.parse(await read('../contracts/openapi/school-api.json'));
	const api = await read('src/lib/api/timetable.ts');
	const boardState = await read('src/lib/academic/timetable/board-state.ts');
	const controller = await read('src/lib/academic/timetable/workspace-controller.svelte.ts');

	const source = openapi.components.schemas.TimetableBlockPlacementSource;
	assert.deepEqual(
		source.oneOf.map((variant) => variant.properties.kind.enum[0]).toSorted(),
		['existing_block', 'ordinary_demand', 'synchronized_offering'].toSorted()
	);
	assert.match(api, /previewTimetableBlockPlacement/);
	assert.match(boardState, /blockTeacherIds/);
	assert.match(boardState, /blockHomeroomIds/);
	assert.match(boardState, /block\.groups/);
	assert.match(controller, /\$state\.raw/);
	assert.match(controller, /\$derived/);
});

test('timetable workspace exposes ranked preferred rooms for each learning group', async () => {
	const openapi = JSON.parse(await read('../contracts/openapi/school-api.json'));
	const learningGroup = openapi.components.schemas.TimetableBlockWorkspaceLearningGroup;

	assert.ok(learningGroup.required.includes('preferredRoomIds'));
	assert.equal(learningGroup.properties.preferredRoomIds.type, 'array');
	assert.equal(learningGroup.properties.preferredRoomIds.items.format, 'uuid');
});

test('date-based personal timetable reads carry the selected date', async () => {
	const api = await read('src/lib/api/timetable.ts');
	const parents = await read('src/lib/api/parents.ts');
	assert.match(api, /date:\s*requiredDate/);
	assert.match(parents, /date:\s*requiredDate/);
});

test('personal timetable and PDF view models use canonical block naming', async () => {
	const sources = await Promise.all([
		read('src/routes/(app)/staff/timetable/+page.svelte'),
		read('src/routes/(app)/student/timetable/+page.svelte'),
		read('src/routes/(app)/parent/student/[id]/timetable/+page.svelte'),
		read('src/lib/utils/pdf.ts'),
		read('src/lib/utils/staff-own-timetable-pdf.ts')
	]);
	const combined = sources.join('\n');

	assert.match(combined, /timetableBlocks|\bblocks\b/);
	assert.doesNotMatch(combined, /timetableEntries|let entries = \$state<TimetableBlock/);
});

test('whole-school view derives from the same canonical workspace without a second endpoint', async () => {
	const page = await read('src/routes/(app)/staff/academic/timetable/+page.svelte');
	const api = await read('src/lib/api/timetable.ts');
	assert.match(page, /activeView === 'wholeSchool'/);
	assert.match(page, /controller\.workspace\.homerooms/);
	assert.match(page, /controller\.workspace\.blocks\.filter/);
	assert.match(page, /มุมมองนี้ใช้ตรวจภาพรวม/);
	assert.doesNotMatch(api, /getWholeSchoolTimetableOverview/);
});

test('current singleton contract has explicit creation and no source-alignment or publication endpoints', async () => {
 const loader=await read('src/routes/(app)/staff/academic/timetable/+page.ts');
 const openapi=JSON.parse(await read('../contracts/openapi/school-api.json'));
 assert.equal(openapi.paths['/api/academic/timetable'].get.operationId,'getTimetable');
 assert.equal(openapi.paths['/api/academic/timetable'].post.operationId,'createTimetable');
 assert.deepEqual(openapi.components.schemas.CreateTimetableRequest.required,['academicTermId']);
 for(const path of Object.keys(openapi.paths))assert.doesNotMatch(path,/timetable-versions|timetable.*delivery-source|timetable.*publication-preview/);
 assert.doesNotMatch(loader,/createTimetable|updateTimetableDeliverySource/);
});

test('unknown dispatched saves stay fenced until a deliberate authoritative read without automatic replay', async () => {
 const page=await read('src/routes/(app)/staff/academic/timetable/+page.svelte');
 assert.match(page,/outcome: mutationSent \? 'unknown' : 'not_submitted'/);
 assert.match(page,/await waitForSaves\(\)/);
 const discard=page.slice(page.indexOf('async function discardFailedSave'),page.indexOf('function lockedTeacherTargetIds'));
 assert.match(discard,/await reload/);assert.doesNotMatch(discard,/createOrdinaryTimetableBlock|updateTimetableBlock|swapTimetableBlocks/);
 assert.doesNotMatch(page,/TIMETABLE_DELIVERY_SOURCE_OUTDATED|ensureLatestDeliverySource|updateTimetableDeliverySource/);
});
