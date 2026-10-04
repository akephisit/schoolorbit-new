import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const readProjectFile = (relativePath) => readFile(path.join(projectRoot, relativePath), 'utf8');

test('operational academic change API consumes generated contracts and camelCase queries', async () => {
	const api = await readProjectFile('src/lib/api/learning-delivery.ts');

	for (const operation of [
		'listAcademicTermChangeSets',
		'createAcademicTermChangeSet',
		'getAcademicTermChangeSet',
		'updateAcademicTermChangeSet',
		'deleteDeliveryVersion',
		'upsertAcademicTermChangeItem',
		'deleteAcademicTermChangeItem',
		'previewAcademicTermChangeSet',
		'publishAcademicTermChangeSet',
		'listDatedRosterMemberships',
		'addDatedRosterMembership',
		'endDatedRosterMembership'
	]) {
		assert.match(api, new RegExp(`operations\\['${operation}'\\]`));
	}
	assert.match(api, /academicTermId:\s*selectedTerm\(academicTermId\)/);
	assert.match(api, /satisfies ListAcademicTermChangeSetsQuery/);
	assert.match(api, /apiClient\.get<AcademicTermChangeSetSummary\[]>/);
	assert.match(api, /apiClient\.get<AcademicTermChangeSet>\(changeSetPath\(id\)/);
	assert.match(api, /deleteWithBody<AcademicTermChangeSet>/);
	assert.doesNotMatch(api, /academic_term_id|ApiResponse<unknown>|Record<string, unknown>/);
});

test('delivery versions expose immutable source data and permission-gated version creation', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/delivery/+page.svelte');
	const table = await readProjectFile(
		'src/lib/components/learning-delivery/DeliveryVersionOverviewTable.svelte'
	);

	assert.match(page, /AcademicChangeSetDialog/);
	assert.match(page, /{#if canManage[\s\S]*<AcademicChangeSetDialog/);
	assert.match(page, /AcademicChangeSetPanel/);
	assert.match(page, /selectedChangeSetId/);
	assert.match(page, /deliveryVersionLabel/);
	assert.match(page, /selectedDelivery/);
	assert.match(page, /updated\.items\.length > 0/);
	assert.match(page, /const routeResult = data\.homerooms/);
	assert.match(page, /const routeResult = data\.changeSetSummaries/);
	assert.match(page, /const routeResult = data\.selectedChangeSet/);
	assert.doesNotMatch(page, /applyPageView|data\.pageView/);
	assert.match(table, /version.snapshot.offerings/);
	assert.match(table, /deliveryVersionId: version.id/);
	assert.match(table, /weeklyPeriodTarget/);
	assert.match(page, /สร้างรุ่นเปิดสอน/);
});

test('change creation is explicit, exceptional, and keeps curriculum unchanged', async () => {
	const dialog = await readProjectFile(
		'src/lib/components/learning-delivery/AcademicChangeSetDialog.svelte'
	);

	assert.doesNotMatch(dialog, /DatePicker/);
	assert.doesNotMatch(dialog, /effectiveFrom/);
	assert.match(dialog, /reason/);
	assert.match(dialog, /รายวิชา กลุ่ม ครู/);
	assert.match(dialog, /รุ่นเปิดสอน/);
	assert.match(dialog, /disabled={!reason\.trim\(\)/);
});

test('opening readiness is independent of timetable publication', async () => {
	const panel = await readProjectFile(
		'src/lib/components/learning-delivery/AcademicChangeSetPanel.svelte'
	);
	const readiness = await readProjectFile(
		'src/lib/components/learning-delivery/AcademicChangeReadiness.svelte'
	);

	assert.match(panel, /DeliveryOptionCombobox/);
	assert.match(panel, /standardPeriodsPerWeek/);
	assert.match(panel, /ตามหลักสูตร/);
	assert.match(panel, /จัดจริงภาคเรียนนี้/);
	assert.match(panel, /weeklyPeriodTarget/);
	assert.match(panel, /AcademicChangeReadiness/);
	assert.doesNotMatch(
		panel,
		/previewAcademicTermChangeSet|publishAcademicTermChangeSet|cancelAcademicTermChangeSet/
	);
	assert.match(readiness, /preview\?\.findings\.filter/);
	assert.match(readiness, /acknowledgedWarnings/);
	assert.match(readiness, /onCheckedChange/);
	assert.match(readiness, /DatePicker/);
	assert.match(readiness, /preview\.effectiveFrom === selectedDate/);
	assert.match(readiness, /effectiveFrom: selectedDate/);
	assert.match(readiness, /clearPreview/);
	assert.match(readiness, /deleteDeliveryVersion/);
	assert.doesNotMatch(
		readiness,
		/cancelAcademicTermChangeSet|targetTimetableVersionId|weekly_period_excess|stopped_teacher_still_scheduled/
	);
	assert.match(panel, /changeSet\.changes/);
	assert.match(panel, /changeSet\.offeringLabels/);
	assert.match(readiness, /blocking\.length === 0/);
});

test('post-publication roster uses dated interval history with inclusive end semantics', async () => {
	const detail = await readProjectFile(
		'src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte'
	);
	const roster = await readProjectFile(
		'src/lib/components/learning-delivery/DatedRosterMemberships.svelte'
	);

	assert.match(detail, /DatedRosterMemberships/);
	assert.match(detail, /rosterStatus === 'published'/);
	assert.match(roster, /joinedAt/);
	assert.match(roster, /leftAt/);
	assert.match(roster, /DatePicker/);
	assert.match(roster, /รวมวันสิ้นสุด|นับรวมวันสิ้นสุด/);
	assert.match(roster, /กำลังจะเริ่ม|กำลังเรียน|สิ้นสุดแล้ว/);
	assert.match(roster, /if \(!canManage\) return/);
	const loadHistorySource = roster.slice(
		roster.indexOf('async function loadHistory'),
		roster.indexOf('async function showAddForm')
	);
	const showAddFormSource = roster.slice(
		roster.indexOf('async function showAddForm'),
		roster.indexOf('async function addMembership')
	);
	assert.match(loadHistorySource, /listDatedRosterMemberships/);
	assert.doesNotMatch(loadHistorySource, /previewLearningGroupRoster/);
	assert.match(showAddFormSource, /if \(!canManage\) return/);
	assert.match(showAddFormSource, /previewLearningGroupRoster/);
	assert.match(roster, /ApiClientError/);
	assert.match(roster, /status === 409/);
	assert.match(roster, /recoverFromConflict[\s\S]*onGroupChanged\(\)[\s\S]*loadHistory\(\)/);
	assert.doesNotMatch(roster, /national|บัตรประชาชน/);
});

test('published teachers stay locked and exceptional changes use the typed change-set workflow', async () => {
	const detail = await readProjectFile(
		'src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte'
	);
	const panel = await readProjectFile(
		'src/lib/components/learning-delivery/AcademicChangeSetPanel.svelte'
	);

	assert.match(detail, /teachersLocked/);
	assert.match(detail, /ครูผู้สอนถูกล็อกแล้ว/);
	assert.match(detail, /offering\?\.status === 'cancelled' \|\| offering\?\.status === 'closed'/);
	assert.match(detail, /canManage={canMutateOffering}/);
	assert.match(detail, /if \(!canMutateOffering/);
	assert.match(panel, /changeSet\.status === 'draft' \? 'จัดกลุ่มและครู' : 'ดูรายละเอียด'/);
	assert.match(panel, /AcademicTeacherChangeForm/);
	assert.match(panel, /TeacherHandoffPanel/);
	assert.doesNotMatch(panel, /replaceLearningGroupTeachers/);
});
