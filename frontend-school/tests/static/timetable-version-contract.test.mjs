import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const read = (relativePath) => readFile(path.join(projectRoot, relativePath), 'utf8');

test('canonical timetable operations require explicit term and version context', async () => {
	const openapi = JSON.parse(await read('../contracts/openapi/school-api.json'));
	const requiredVersionSchemas = [
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
	for (const schemaName of requiredVersionSchemas) {
		assert.ok(
			openapi.components.schemas[schemaName].required.includes('timetableVersionId'),
			`${schemaName} must require timetableVersionId`
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

test('academic timetable keeps one URL-backed version and published boards read-only', async () => {
	const page = await read('src/routes/(app)/staff/academic/timetable/+page.svelte');
	const header = await read(
		'src/lib/components/academic/timetable/TimetableWorkspaceHeader.svelte'
	);

	assert.match(page, /listTimetableVersions\(/);
	assert.match(page, /searchParams\.set\('timetableVersionId'/);
	assert.match(page, /controller\?\.canEdit/);
	assert.match(page, /window\.history\.replaceState/);
	assert.match(header, /เผยแพร่แล้ว/);
	assert.match(header, /อ่านอย่างเดียว/);
	assert.match(page, /timetableVersionId:\s*controller\.workspace\.version\.id/);
});

test('published timetable revisions and teacher load export remain available after block cutover', async () => {
	const page = await read('src/routes/(app)/staff/academic/timetable/+page.svelte');

	assert.match(page, /getAcademicTermChangeSet/);
	assert.match(page, /AcademicChangeSetDialog/);
	assert.match(page, /purpose="timetable_revision"/);
	assert.match(page, /AcademicChangeReadiness/);
	assert.match(page, /downloadTeacherLoadWorkbook/);
	assert.match(page, /controller\.workspace\.blocks/);
});

test('placement preview does not disable the active drop target', async () => {
	const page = await read('src/routes/(app)/staff/academic/timetable/+page.svelte');

	assert.match(page, /controller\?\.canEdit\s*&&\s*!busy\)/);
	assert.doesNotMatch(page, /controller\?\.canEdit\s*&&\s*!busy\s*&&\s*!previewing/);
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
