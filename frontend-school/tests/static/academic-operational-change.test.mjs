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
		'cancelAcademicTermChangeSet',
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
	assert.match(api, /deleteWithBody<AcademicTermChangeSet>/);
	assert.doesNotMatch(api, /academic_term_id|ApiResponse<unknown>|Record<string, unknown>/);
});

test('published delivery rows expose date-derived state and a permission-gated exceptional action', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/delivery/+page.svelte');
	const table = await readProjectFile(
		'src/lib/components/learning-delivery/OfferingOverviewTable.svelte'
	);

	assert.match(page, /AcademicChangeSetDialog/);
	assert.match(page, /{#if canManage[\s\S]*<AcademicChangeSetDialog/);
	assert.match(page, /AcademicChangeSetPanel/);
	assert.match(page, /selectedChangeSetId/);
	assert.match(page, /เลือกดูแบบร่างที่กำลังทำหรือประวัติที่เผยแพร่และยกเลิกแล้ว/);
	assert.match(page, /changeSet\.items\.length > 0/);
	assert.match(
		page,
		/contextKey !== loadedContext[\s\S]*workspace = null;[\s\S]*changeSets = \[\];[\s\S]*selectedChangeSetId = '';[\s\S]*loadWorkspace/
	);
	assert.match(table, /startsOn/);
	assert.match(table, /endsOn/);
	assert.match(table, /กำลังจะเริ่ม|กำลังสอน|สิ้นสุดแล้ว/);
	assert.match(page, /เพิ่ม\/ปรับ\/หยุดกลางภาค/);
});

test('change creation is explicit, exceptional, and keeps curriculum unchanged', async () => {
	const dialog = await readProjectFile(
		'src/lib/components/learning-delivery/AcademicChangeSetDialog.svelte'
	);

	assert.match(dialog, /DatePicker/);
	assert.match(dialog, /effectiveFrom/);
	assert.match(dialog, /reason/);
	assert.match(dialog, /ไม่เปลี่ยนหลักสูตร/);
	assert.match(dialog, /เฉพาะภาคเรียนนี้/);
	assert.match(dialog, /disabled={!effectiveFrom\.trim\(\) \|\| !reason\.trim\(\)/);
});

test('change panel separates readiness, impacts, scheduling, and warning acknowledgement', async () => {
	const panel = await readProjectFile(
		'src/lib/components/learning-delivery/AcademicChangeSetPanel.svelte'
	);

	assert.match(panel, /DeliveryOptionCombobox/);
	assert.match(panel, /standardPeriodsPerWeek/);
	assert.match(panel, /ตามหลักสูตร/);
	assert.match(panel, /จัดจริงภาคเรียนนี้/);
	assert.match(panel, /weeklyPeriodTarget/);
	assert.match(panel, /blockingFindings/);
	assert.match(panel, /warningFindings/);
	assert.match(panel, /acknowledgedWarnings/);
	assert.match(panel, /onCheckedChange/);
	assert.doesNotMatch(panel, /<Checkbox[\s\S]{0,300}\sonchange=/);
	assert.match(panel, /new Set\(warningFindings\.map\(\(finding\) => finding\.code\)\)/);
	assert.match(panel, /weekly_period_excess/);
	assert.match(
		panel,
		/getAcademicTermChangeSet\(changeSet\.id\)[\s\S]*previewAcademicTermChangeSet\(changeSet\.id\)/
	);
	assert.match(panel, /{#if changeSet\.status === 'draft'}[\s\S]*ตรวจผลกระทบและความพร้อม/);
	assert.match(panel, /ชุดนี้เผยแพร่แล้ว/);
	assert.match(panel, /แบบร่างนี้ยกเลิกแล้ว/);
	assert.match(
		panel,
		/recoverFromConflict[\s\S]*preview = null;[\s\S]*acknowledgedWarnings = \[\]/
	);
	assert.match(
		panel,
		/ตาราง|กลุ่มเรียน|รายชื่อนักเรียน|ครูผู้สอน|โครงสร้างคะแนน|ผลการเรียน|ตารางสอบ|นิเทศ/
	);
	assert.match(panel, /ข้อมูลเดิมยังคงอยู่/);
	assert.match(panel, /\/staff\/academic\/timetable\?timetableVersionId=/);
	assert.match(panel, /blockingFindings\.length > 0|warningsAcknowledged/);
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

test('published teachers remain locked while exceptional workflow offers no replacement action', async () => {
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
	assert.doesNotMatch(panel, /replaceLearningGroupTeachers|เปลี่ยนครูผู้สอน/);
});
