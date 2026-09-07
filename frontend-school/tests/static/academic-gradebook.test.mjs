import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');

async function read(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

const pagePath = 'src/routes/(app)/staff/academic/gradebook/+page.svelte';

test('gradebook route is term-scoped and discoverable to score or evaluation readers', async () => {
	const meta = await read('src/routes/(app)/staff/academic/gradebook/+page.ts');

	assert.match(meta, /academicContext:\s*'term_required'/);
	assert.match(meta, /workspace:\s*'academic'/);
	assert.match(meta, /group:\s*'academic_assessment'/);
	for (const permission of [
		'ACADEMIC_GRADEBOOK_READ_ASSIGNED',
		'ACADEMIC_GRADEBOOK_READ_ORGANIZATION_UNIT',
		'ACADEMIC_GRADEBOOK_READ_SCHOOL',
		'ACADEMIC_LEARNER_EVALUATION_READ_ASSIGNED',
		'ACADEMIC_LEARNER_EVALUATION_READ_ORGANIZATION_UNIT',
		'ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL'
	]) {
		assert.match(meta, new RegExp(`PERMISSIONS\\.${permission}`));
	}
});

test('gradebook workspace keeps URL-backed subject, group, tab, and phase state', async () => {
	const page = await read(pagePath);

	for (const key of ['subjectId', 'learningGroupId', 'tab', 'phase']) {
		assert.match(page, new RegExp(`searchParams\\.(?:get|set)\\('${key}'`));
	}
	assert.match(page, /goto\(/);
	assert.match(page, /replaceState:\s*true/);
	assert.match(page, /LatestRequest/);
	assert.match(page, /\$state\.raw<GradebookSubject\[\]>/);
	assert.match(page, /\$state\.raw<LearnerEvaluationSubject\[\]>/);
});

test('gradebook presents three entry tabs and four fixed score phases', async () => {
	const page = await read(pagePath);

	for (const label of [
		'คะแนนรายวิชา',
		'คุณลักษณะอันพึงประสงค์',
		'การอ่าน คิดวิเคราะห์ และเขียน',
		'ก่อนกลางภาค',
		'กลางภาค',
		'หลังกลางภาค',
		'ปลายภาค'
	]) {
		assert.match(page, new RegExp(label));
	}
	for (const component of [
		'GradebookWorkspaceHeader',
		'GradebookEntryControls',
		'ScoreLedger',
		'LearnerEvaluationLedger',
		'GradebookMobileEditor',
		'PhaseConfirmationDialog'
	]) {
		assert.match(page, new RegExp(component));
	}
});

test('desktop score ledger is a checkbox-selected, keyboard and paste-enabled matrix', async () => {
	const ledger = await read('src/lib/components/academic/gradebook/ScoreLedger.svelte');

	assert.match(ledger, /overflow-x-auto/);
	assert.match(ledger, /sticky left-0/);
	assert.match(ledger, /<Checkbox/);
	assert.match(ledger, /selectedItemIds/);
	assert.match(ledger, /nextEditableCell/);
	assert.match(ledger, /normalizeScorePaste/);
	assert.match(ledger, /onkeydown/);
	assert.match(ledger, /onpaste/);
	assert.match(ledger, /เพิ่มรายการคะแนน/);
	assert.match(ledger, /เลือกทุกช่อง/);
	assert.match(ledger, /ล้างการเลือก/);
});

test('gradebook save state remains sticky and mobile editor has an explicit close action', async () => {
	const page = await read(pagePath);
	const mobile = await read('src/lib/components/academic/gradebook/GradebookMobileEditor.svelte');

	assert.match(page, /createGradebookSaveQueue/);
	assert.match(page, /registerAcademicContextDirtySource/);
	assert.match(page, /flushPendingWork/);
	assert.match(page, /retry/);
	assert.match(page, /discard/);
	assert.match(mobile, /showCloseButton=\{false\}/);
	assert.match(mobile, /aria-label="ปิดหน้ากรอกคะแนน"/);
	assert.match(mobile, /sticky top-0/);
	assert.match(mobile, /sticky bottom-0/);
});

test('action-only control requests are permission-gated while readers retain the workspace', async () => {
	const page = await read(pagePath);
	const managerBlock = page.slice(
		page.indexOf('async function loadManagerControls'),
		page.indexOf('async function loadSubjects')
	);

	assert.match(page, /canManageGradebookSchool/);
	assert.match(page, /canManageEvaluationSchool/);
	assert.match(
		managerBlock,
		/if \(!canManageGradebookSchool && !canManageEvaluationSchool\) return/
	);
	assert.match(managerBlock, /listGradebookControls/);
	assert.match(managerBlock, /listLearnerEvaluationControls/);
	assert.match(page, /\{#if canManageGradebookSchool \|\| canManageEvaluationSchool\}/);
});

test('learner evaluation ledger uses local criterion selection and exact 0 through 3 values', async () => {
	const ledger = await read('src/lib/components/academic/gradebook/LearnerEvaluationLedger.svelte');

	assert.match(ledger, /selectedCriterionIds/);
	assert.match(ledger, /<Checkbox/);
	for (const label of ['3 · ดีเยี่ยม', '2 · ดี', '1 · ผ่าน', '0 · ไม่ผ่าน', 'ยังไม่ประเมิน']) {
		assert.match(ledger, new RegExp(label));
	}
	assert.match(ledger, /บันทึกผลประเมิน/);
	assert.match(ledger, /จัดหัวข้อประเมิน/);
});

test('assessment page links to gradebook status without mutating gradebook controls', async () => {
	const page = await read('src/routes/(app)/staff/academic/assessments/+page.svelte');

	assert.match(page, /ACADEMIC_GRADEBOOK_READ_/);
	assert.match(page, /\/staff\/academic\/gradebook/);
	assert.match(page, /ไปหน้ากรอกคะแนน/);
	assert.doesNotMatch(page, /updateGradebookControl|listGradebookControls/);
});
