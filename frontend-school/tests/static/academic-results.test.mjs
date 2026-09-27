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
	assert.match(component, /desirable_characteristic/);
	assert.match(component, /คุณลักษณะอันพึงประสงค์/);
	assert.match(component, /การอ่าน คิดวิเคราะห์ และเขียน/);
	assert.match(page, /getLearnerEvaluationLockReadiness/);
	assert.doesNotMatch(page, /listLearnerEvaluationSubjects/);
	assert.match(component, /resultBlockerLabel/);
	assert.match(component, /group\.groupName/);
	assert.match(component, /!row\.ready/);
	assert.doesNotMatch(component, /ตรวจความครบถ้วนเมื่อกดล็อก/);
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

test('result preparation route owns independent visible regions and keeps inactive sections lazy', async () => {
	const [route, page] = await Promise.all([
		read('src/routes/(app)/staff/academic/results/+page.ts'),
		read('src/routes/(app)/staff/academic/results/+page.svelte')
	]);
	assert.match(route, /captureRouteLoad/);
	assert.match(route, /requestFetch: fetch/);
	assert.match(route, /getAcademicResultReadiness/);
	assert.match(route, /listAcademicGradingPolicies/);
	assert.match(route, /listLearnerEvaluationSubjects/);
	assert.match(page, /data\.overview/);
	assert.match(page, /data\.policy/);
	assert.doesNotMatch(page, /onMount\(/);
});

test('confirmation retains the selected workspace and retries readiness alone', async () => {
	const page = await read('src/routes/(app)/staff/academic/results/+page.svelte');
	const confirmation = page.slice(
		page.indexOf('async function confirmCourse'),
		page.indexOf('async function changeStudent')
	);
	assert.match(confirmation, /refreshReadiness\(/);
	assert.doesNotMatch(confirmation, /loadOverview\(/);
	assert.match(page, /readinessError/);
	assert.match(page, /aria-busy=\{readinessUpdating\}/);
});

test('result lock and correction routes own their initial reads', async () => {
	const [lockRoute, lockPage, correctionRoute, correctionPage] = await Promise.all([
		read('src/routes/(app)/staff/academic/result-locks/+page.ts'),
		read('src/routes/(app)/staff/academic/result-locks/+page.svelte'),
		read('src/routes/(app)/staff/academic/result-corrections/+page.ts'),
		read('src/routes/(app)/staff/academic/result-corrections/+page.svelte')
	]);
	assert.match(lockRoute, /captureRouteLoad/);
	assert.match(lockRoute, /requestFetch: fetch/);
	assert.match(lockPage, /data\.queue/);
	assert.match(correctionRoute, /searchEffectiveAcademicResults/);
	assert.match(correctionRoute, /requestFetch: fetch/);
	assert.match(correctionPage, /data\.results/);
	assert.doesNotMatch(lockPage, /onMount\(/);
	assert.doesNotMatch(correctionPage, /onMount\(/);
});

test('result lock mutations patch their owning queue without refetching every tab', async () => {
	const page = await read('src/routes/(app)/staff/academic/result-locks/+page.svelte');
	const mutations = page.slice(
		page.indexOf('async function lockCourse'),
		page.indexOf('</script>')
	);
	assert.doesNotMatch(mutations, /loadQueue\(/);
	assert.match(mutations, /outcome\.groups/);
	assert.match(mutations, /outcome\.locked/);
	assert.match(mutations, /outcome\.skipped/);
});

test('late correction responses cannot patch a different academic context', async () => {
	const page = await read('src/routes/(app)/staff/academic/result-corrections/+page.svelte');
	const mutation = page.slice(
		page.indexOf('async function correctResult'),
		page.indexOf('async function refreshSelected')
	);
	assert.match(mutation, /context\.academicTermId\s*!==\s*academicTermId/);
	assert.match(mutation, /context\.academicYearId\s*!==\s*academicYearId/);
});

test('late lock responses cannot patch a different academic year with a reused term id', async () => {
	const page = await read('src/routes/(app)/staff/academic/result-locks/+page.svelte');
	for (const mutation of ['lockCourse', 'lockActivity', 'lockAllActivities', 'lockEvaluation']) {
		const body = page.slice(
			page.indexOf(`async function ${mutation}`),
			page.indexOf('async function', page.indexOf(`async function ${mutation}`) + 1)
		);
		assert.match(body, /context\.academicYearId\s*!==\s*academicYearId/, mutation);
	}
});

test('large result workflow reads require tap rather than incidental hover preload', async () => {
	const sidebar = await read('src/lib/components/layout/Sidebar.svelte');
	assert.match(sidebar, /path === '\/staff\/academic\/result-locks'/);
	assert.match(sidebar, /path === '\/staff\/academic\/result-corrections'/);
});

test('correction empty-state handoff keeps the selected year and term', async () => {
	const page = await read('src/routes/(app)/staff/academic/result-corrections/+page.svelte');
	assert.match(
		page,
		/href: `\/staff\/academic\/result-locks\?academicYearId=\$\{academicYearId\}&academicTermId=\$\{academicTermId\}`/
	);
	assert.match(page, /preload: 'tap'/);
});
