import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');

const readProjectFile = (relativePath) => readFile(path.join(projectRoot, relativePath), 'utf8');

test('delivery workspace uses generated term query contracts', async () => {
	const api = await readProjectFile('src/lib/api/learning-delivery.ts');
	assert.match(api, /operations\['getLearningDeliveryOverview'\]/);
	assert.match(api, /operations\['getHomeroomDeliveryWorkspace'\]/);
	assert.match(api, /operations\['getLearningDeliveryManagementOptions'\]/);
	assert.match(api, /getLearningDeliveryOverview/);
	assert.match(api, /getHomeroomDeliveryWorkspace/);
	assert.match(api, /getLearningDeliveryManagementOptions/);
	assert.match(api, /getLearningOffering/);
	assert.match(api, /getLearningGroup/);
	assert.doesNotMatch(api, /academic_term_id|ApiResponse<unknown>|Record<string, unknown>/);
});

test('homeroom delivery contract is camelCase and preparation requires reviewed choices', async () => {
	const openapi = JSON.parse(await readProjectFile('../contracts/openapi/school-api.json'));
	const operation = openapi.paths['/api/academic/delivery/homerooms'].get;
	assert.equal(operation.operationId, 'getHomeroomDeliveryWorkspace');
	assert.deepEqual(operation.parameters.map((parameter) => parameter.name).sort(), [
		'academicTermId',
		'academicYearId'
	]);
	const preview = openapi.components.schemas.CurriculumOfferingPreview;
	assert.ok(preview.required.includes('proposals'));
	assert.equal(preview.properties.items, undefined);
	const proposal = openapi.components.schemas.CurriculumPreparationProposal;
	assert.ok(proposal.required.includes('defaultGroups'));
	const apply = openapi.components.schemas.ApplyCurriculumOfferingsRequest;
	assert.ok(apply.required.includes('choices'));
});

test('applying a curriculum proposal always requires at least one reviewed group', async () => {
	const preview = await readProjectFile(
		'src/lib/components/learning-delivery/OfferingCurriculumPreview.svelte'
	);

	assert.match(preview, /choice\.action === 'apply'\s*&&\s*choice\.groups\.length === 0/);
});

test('delivery workspace is homeroom-first, loads offering overview lazily, and keeps management options lazy', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/delivery/+page.svelte');
	const table = await readProjectFile(
		'src/lib/components/learning-delivery/OfferingOverviewTable.svelte'
	);
	const dialog = await readProjectFile(
		'src/lib/components/learning-delivery/OfferingCreateDialog.svelte'
	);
	const homerooms = await readProjectFile(
		'src/lib/components/learning-delivery/HomeroomDeliveryWorkspace.svelte'
	);
	const preparation = await readProjectFile(
		'src/lib/components/learning-delivery/OfferingCurriculumPreview.svelte'
	);
	assert.match(page, /getHomeroomDeliveryWorkspace/);
	assert.match(page, /viewMode = \$state<'homerooms' \| 'offerings'>\('homerooms'\)/);
	assert.match(page, /getLearningDeliveryOverview/);
	assert.match(page, /listAcademicTermChangeSets/);
	assert.match(page, /{#if canManage[\s\S]*AcademicChangeSetDialog/);
	assert.match(page, /academicTermId/);
	assert.match(page, /kind=activity|kindFilter|initialKind/);
	assert.doesNotMatch(page, /getLearningDeliveryManagementOptions\([\s\S]*onMount/);
	assert.doesNotMatch(dialog, /catalogVersionId[^\n]*<Input/);
	assert.doesNotMatch(dialog, /gradeLevelId[^\n]*<Input/);
	assert.match(homerooms, /room\.items/);
	assert.match(homerooms, /workspace\.unlinked/);
	assert.match(preparation, /proposal\.defaultGroups/);
	assert.match(preparation, /choices/);
	assert.match(preparation, /combineGroups/);
	assert.match(preparation, /addSplitGroup/);
	assert.match(table, /groupsWithoutPrimaryTeacher/);
	assert.match(table, /publishedRosterCount/);
});

test('offering detail keeps selection in the URL and renders named group and roster data', async () => {
	const meta = await readProjectFile(
		'src/routes/(app)/staff/academic/delivery/[offeringId]/+page.ts'
	);
	const page = await readProjectFile(
		'src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte'
	);
	const editor = await readProjectFile(
		'src/lib/components/learning-delivery/LearningGroupEditor.svelte'
	);
	const roster = await readProjectFile(
		'src/lib/components/learning-delivery/RosterPreviewPanel.svelte'
	);
	assert.match(meta, /access:/);
	assert.doesNotMatch(meta, /menu:/);
	assert.match(page, /getLearningOffering/);
	assert.match(page, /getLearningGroup/);
	assert.match(page, /listLearningGroups/);
	assert.match(page, /groupId/);
	assert.match(page, /rosterStatus === 'published'[\s\S]*DatedRosterMemberships/);
	assert.match(editor, /managementOptions\.teachers/);
	assert.match(editor, /managementOptions\.homerooms/);
	assert.match(editor, /managementOptions\.rooms/);
	assert.match(roster, /studentCode/);
	assert.match(roster, /displayName/);
	assert.match(roster, /gradeLevelName/);
	assert.match(roster, /homeroomName/);
	assert.doesNotMatch(roster, /student\.studentId\s*\}/);
});

test('delivery workload separates the official standard from the current-term target', async () => {
	const page = await readProjectFile(
		'src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte'
	);
	const homerooms = await readProjectFile(
		'src/lib/components/learning-delivery/HomeroomDeliveryWorkspace.svelte'
	);
	const table = await readProjectFile(
		'src/lib/components/learning-delivery/OfferingOverviewTable.svelte'
	);

	assert.doesNotMatch(page, /weeklyPeriodTarget\s*:/);
	assert.match(page, /selectedTimetableTarget/);
	for (const source of [page, homerooms]) {
		assert.match(source, /ตามหลักสูตร/);
		assert.match(source, /จัดจริงภาคเรียนนี้/);
	}
	assert.match(table, /ตามหลักสูตร/);
	assert.doesNotMatch(table, /snapshot\.weeklyPeriodTarget/);
});

test('published learning groups keep teacher names visible without mutation controls', async () => {
	const page = await readProjectFile(
		'src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte'
	);
	const editor = await readProjectFile(
		'src/lib/components/learning-delivery/LearningGroupEditor.svelte'
	);

	assert.match(page, /teachersLocked/);
	assert.match(editor, /teachersLocked/);
	assert.match(editor, /teacherAssignments/);
	assert.match(editor, /ไม่สามารถเปลี่ยนครูผู้สอนได้/);
});

test('stale roster refresh replaces the group version and preview atomically', async () => {
	const page = await readProjectFile(
		'src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte'
	);
	const refreshStart = page.indexOf('async function refreshRoster()');
	const applyStart = page.indexOf('async function applyRoster(', refreshStart);
	assert.notEqual(refreshStart, -1);
	assert.notEqual(applyStart, -1);
	const refresh = page.slice(refreshStart, applyStart);
	const reloadIndex = refresh.indexOf('await getLearningGroup(');
	const previewIndex = refresh.indexOf('await previewLearningGroupRoster(');
	const updateIndex = refresh.indexOf('updateGroupState(');

	assert.notEqual(reloadIndex, -1);
	assert.notEqual(previewIndex, -1);
	assert.notEqual(updateIndex, -1);
	assert.ok(reloadIndex < previewIndex);
	assert.ok(previewIndex < updateIndex);
	assert.match(refresh, /rosterPreview\s*=\s*refreshedPreview/);
});
