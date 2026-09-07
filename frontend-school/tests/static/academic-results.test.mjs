import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const read = (path) => readFile(new URL(`../../${path}`, import.meta.url), 'utf8');

test('result preparation is term scoped and accepts result or learner-evaluation readers', async () => {
	const route = await read('src/routes/(app)/staff/academic/results/+page.ts');
	assert.match(route, /academicContext:\s*'term_required'/);
	for (const permission of [
		'ACADEMIC_RESULT_READ_ASSIGNED',
		'ACADEMIC_RESULT_READ_ORGANIZATION_UNIT',
		'ACADEMIC_RESULT_READ_SCHOOL',
		'ACADEMIC_LEARNER_EVALUATION_READ_ASSIGNED',
		'ACADEMIC_LEARNER_EVALUATION_READ_ORGANIZATION_UNIT',
		'ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL'
	]) {
		assert.match(route, new RegExp(`PERMISSIONS\\.${permission}`));
	}
});

test('lock and correction routes use their exact school permissions', async () => {
	const [locks, corrections] = await Promise.all([
		read('src/routes/(app)/staff/academic/result-locks/+page.ts'),
		read('src/routes/(app)/staff/academic/result-corrections/+page.ts')
	]);
	assert.match(locks, /PERMISSIONS\.ACADEMIC_RESULT_LOCK_SCHOOL/);
	assert.match(locks, /PERMISSIONS\.ACADEMIC_LEARNER_EVALUATION_LOCK_SCHOOL/);
	assert.doesNotMatch(locks, /PERMISSION_MODULES/);
	assert.match(corrections, /PERMISSIONS\.ACADEMIC_RESULT_CORRECT_SCHOOL/);
	assert.match(corrections, /PERMISSIONS\.ACADEMIC_LEARNER_EVALUATION_CORRECT_SCHOOL/);
	assert.doesNotMatch(corrections, /PERMISSION_MODULES/);
});

test('preparation has owned subjects first and only derived, zero, incomplete, or insufficient attendance outcomes', async () => {
	const [page, presentation] = await Promise.all([
		read('src/routes/(app)/staff/academic/results/+page.svelte'),
		read('src/lib/academic/results/presentation.ts')
	]);
	assert.match(page, /sortAssignedFirst/);
	assert.match(presentation, /ตามคะแนน/);
	assert.match(presentation, /'0'/);
	assert.match(presentation, /'ร'/);
	assert.match(presentation, /'มส'/);
	assert.doesNotMatch(page, /อิงกลุ่ม|เปอร์เซ็นไทล์|grading method|method selector/i);
});

test('preparation shows automatic subject readiness, activity blanks, learner summaries, and no redundant submit', async () => {
	const [page, activity] = await Promise.all([
		read('src/routes/(app)/staff/academic/results/+page.svelte'),
		read('src/lib/components/academic/results/ActivityEvaluationTable.svelte')
	]);
	assert.match(page, /ResultPreparationTable/);
	assert.match(page, /ActivityEvaluationTable/);
	assert.match(page, /LearnerEvaluationSummary/);
	assert.match(page, /พร้อมส่งฝ่ายวิชาการอัตโนมัติ/);
	assert.match(activity, /ผลกิจกรรมยังขาด/);
	assert.doesNotMatch(page, />\s*ส่งผลการเรียน\s*</);
});

test('readers never issue mutation-only preparation or lock requests', async () => {
	const [results, locks] = await Promise.all([
		read('src/routes/(app)/staff/academic/results/+page.svelte'),
		read('src/routes/(app)/staff/academic/result-locks/+page.svelte')
	]);
	assert.match(results, /if \(!canManageResult\) return/);
	assert.doesNotMatch(results, /saveLearnerEvaluationResponses|updateLearnerEvaluationControl/);
	assert.match(locks, /if \(!canLockCourseResults\) return/);
	assert.match(locks, /if \(!canLockLearnerEvaluations\) return/);
});

test('lock queue exposes typed blockers and independent learner-evaluation domain locks', async () => {
	const [page, component] = await Promise.all([
		read('src/routes/(app)/staff/academic/result-locks/+page.svelte'),
		read('src/lib/components/academic/results/ResultLockQueue.svelte')
	]);
	assert.match(page, /desirable_characteristic/);
	assert.match(page, /reading_thinking_writing/);
	assert.match(component, /resultBlockerLabel/);
	assert.match(component, /ล็อกผลกิจกรรมที่พร้อมทั้งหมด/);
});

test('correction UI keeps initial and current values visible and appends a new correction', async () => {
	const [page, dialog] = await Promise.all([
		read('src/routes/(app)/staff/academic/result-corrections/+page.svelte'),
		read('src/lib/components/academic/results/ResultCorrectionDialog.svelte')
	]);
	assert.match(page, /searchEffectiveAcademicResults/);
	assert.match(dialog, /ผลเริ่มต้น/);
	assert.match(dialog, /ผลที่ใช้ปัจจุบัน/);
	assert.match(dialog, /ประวัติการแก้ไข/);
	assert.match(dialog, /correctEffectiveAcademicResult|oncorrect/);
	assert.doesNotMatch(dialog, /แก้ไขประวัติ|ลบประวัติ/);
});
