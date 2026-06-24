import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '../..');
const repoRoot = path.resolve(__dirname, '../../..');

async function readProjectFile(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

async function readRepoFile(relativePath) {
	return readFile(path.join(repoRoot, relativePath), 'utf8');
}

test('academic assessment permissions are registered for teachers and academic office', async () => {
	const registry = await readProjectFile('src/lib/permissions/registry.ts');

	assert.match(registry, /ACADEMIC_ASSESSMENT:\s*['"]academic_assessment['"]/);
	assert.match(
		registry,
		/ACADEMIC_ASSESSMENT_READ_ASSIGNED:\s*['"]academic_assessment\.read\.assigned['"]/
	);
	assert.match(
		registry,
		/ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT:\s*['"]academic_assessment\.read\.organization_unit['"]/
	);
	assert.match(
		registry,
		/ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED:\s*['"]academic_assessment\.manage\.assigned['"]/
	);
	assert.match(
		registry,
		/ACADEMIC_ASSESSMENT_READ_SCHOOL:\s*['"]academic_assessment\.read\.school['"]/
	);
	assert.match(
		registry,
		/ACADEMIC_ASSESSMENT_MANAGE_SCHOOL:\s*['"]academic_assessment\.manage\.school['"]/
	);
});

test('academic assessment api client targets the assessment plan endpoints', async () => {
	const source = await readProjectFile('src/lib/api/academicAssessments.ts');

	for (const exportName of [
		'listAssessmentPlans',
		'getAssessmentPlan',
		'saveAssessmentPlan',
		'submitAssessmentPlan',
		'getAssessmentSettings',
		'updateAssessmentSettings'
	]) {
		assert.match(source, new RegExp(`export async function ${exportName}`));
	}

	assert.match(source, /\/api\/academic\/assessments\/plans/);
	assert.match(source, /\/api\/academic\/assessments\/settings/);
	assert.match(source, /\/api\/academic\/assessments\/courses\/\$\{courseId\}/);
	assert.match(source, /\/api\/academic\/assessments\/courses\/\$\{courseId\}\/submit/);
	assert.match(
		source,
		/type AssessmentExamMode = 'none' \| 'in_timetable' \| 'outside_timetable' \| 'practical'/
	);
	assert.match(
		source,
		/type AssessmentPlanStatus = 'not_configured' \| 'draft' \| 'saved' \| 'submitted' \| 'locked'/
	);
});

test('academic assessment route exposes overview, downloads, and quick score editing', async () => {
	const meta = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.ts');
	const page = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.svelte');

	assert.match(meta, /title:\s*['"]โครงสร้างคะแนน['"]/);
	assert.match(meta, /permission:\s*PERMISSION_MODULES\.ACADEMIC_ASSESSMENT/);
	assert.match(meta, /group:\s*['"]academic['"]/);

	assert.match(page, /PageShell/);
	assert.match(page, /Download/);
	assert.match(page, /exportAssessmentReport/);
	assert.match(page, /quickScoreDrafts/);
	assert.match(page, /saveAllQuickScoreRows/);
	assert.match(page, /assessment-score-input/);
	assert.match(page, /assessment-exam-cell/);
	assert.match(page, /quickExamModeOptions/);
	assert.match(page, /outside_timetable/);
	assert.match(page, /canEditAssessmentPlan/);
	assert.match(page, /plan\.canManage/);
	assert.match(page, /ดูอย่างเดียว/);
});

test('academic assessment score table uses dedicated score and exam columns', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.svelte');

	for (const heading of ['ก่อน', 'กลาง', 'หลัง', 'ปลาย']) {
		assert.match(page, new RegExp(`heading:\\s*['"]${heading}['"]`));
	}

	for (const heading of ['สอบกลางภาค', 'สอบปลายภาค']) {
		assert.match(page, new RegExp(`<Table\\.Head>${heading}</Table\\.Head>`));
	}
	for (const heading of ['เวลากลางภาค', 'เวลาปลายภาค']) {
		assert.match(page, new RegExp(`<Table\\.Head class="w-\\[96px\\]">${heading}</Table\\.Head>`));
	}

	const tableHeader = page.slice(page.indexOf('<Table.Header>'), page.indexOf('<Table.Body>'));
	assert.match(
		tableHeader,
		/\{#each quickScoreColumns as column \(column\.field\)\}[\s\S]*สอบกลางภาค[\s\S]*เวลากลางภาค[\s\S]*สอบปลายภาค[\s\S]*เวลาปลายภาค/
	);
	assert.match(page, /\{#each quickScoreColumns as column \(column\.field\)\}/);
	assert.doesNotMatch(page, /preMidtermScoreColumns/);
	assert.doesNotMatch(page, /postMidtermScoreColumns/);
	assert.match(page, /\{column\.heading\}/);
	assert.match(page, /Table\.Root class="min-w-\[1240px\]"/);
	assert.match(page, /w-\[78px\] min-w-\[78px\] px-2 text-right/);
	assert.match(page, /assessment-score-cell w-\[78px\] min-w-\[78px\] px-2/);
	assert.match(page, /assessment-score-input h-8 w-14 min-w-14 px-2 text-right tabular-nums/);
	assert.match(page, /\[appearance:textfield\]/);
	assert.match(page, /\[&::-webkit-inner-spin-button\]:appearance-none/);
	assert.match(page, /\[&::-webkit-outer-spin-button\]:appearance-none/);
	assert.match(page, /<Table\.Cell class="assessment-exam-cell">/);
	assert.match(page, /<Table\.Cell class="assessment-duration-cell w-\[96px\]">/);
	assert.equal(page.match(/<Table\.Cell class="assessment-duration-cell w-\[96px\]">/g)?.length, 2);
	assert.match(page, /<Select\.Trigger class="h-9 text-xs">/);
	assert.doesNotMatch(page, /w-\[72px\] px-2 text-right/);
	assert.doesNotMatch(page, /assessment-score-cell px-2/);
	assert.doesNotMatch(page, /w-\[116px\] px-2/);
	assert.doesNotMatch(page, /w-\[104px\] px-2/);
	assert.doesNotMatch(page, /assessment-exam-cell px-2/);
	assert.doesNotMatch(page, /Select\.Trigger class="h-8 px-2 text-xs"/);
	assert.doesNotMatch(page, /<Table\.Head[^>]*>คะแนน<\/Table\.Head>/);
	assert.match(page, /handleQuickScoreEnter/);
	assert.match(page, /data-assessment-quick-score-input/);
	assert.match(page, /canEditExamDuration/);
	assert.match(page, /mode === 'in_timetable'/);
});

test('academic assessment summary exposes core score buckets for table editing', async () => {
	const api = await readProjectFile('src/lib/api/academicAssessments.ts');
	const model = await readRepoFile('backend-school/src/modules/academic/models/assessment.rs');
	const service = await readRepoFile(
		'backend-school/src/modules/academic/services/assessment_service.rs'
	);

	for (const field of [
		'beforeMidtermScore',
		'midtermScore',
		'afterMidtermScore',
		'finalScore',
		'midtermExamMode',
		'finalExamMode',
		'canManage'
	]) {
		assert.match(api, new RegExp(`${field}[?]?:`));
	}

	for (const field of [
		'before_midterm_score',
		'midterm_score',
		'after_midterm_score',
		'final_score',
		'midterm_exam_mode',
		'final_exam_mode',
		'can_manage'
	]) {
		assert.match(model, new RegExp(`pub ${field}:`));
		assert.match(service, new RegExp(`${field}`));
	}
});

test('academic assessment exposes subject-group read access while keeping row editing scoped', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.svelte');
	const api = await readProjectFile('src/lib/api/academicAssessments.ts');
	const service = await readRepoFile(
		'backend-school/src/modules/academic/services/assessment_service.rs'
	);
	const backendRegistry = await readRepoFile('backend-school/src/permissions/registry.rs');
	const migration = await readRepoFile(
		'backend-school/migrations/015_academic_assessment_subject_group_read.sql'
	);

	assert.match(api, /canManage:\s*boolean/);
	assert.match(page, /PERMISSIONS\.ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT/);
	assert.match(page, /function canEditAssessmentPlan\(plan: AssessmentPlanSummary\)/);
	assert.match(page, /plan\.canManage/);
	assert.match(page, /const dirtyPlans = plans[\s\S]*\.filter\(canEditAssessmentPlan\)/);
	assert.match(page, /ดูอย่างเดียว/);

	assert.match(backendRegistry, /ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT/);
	assert.match(service, /AssessmentPlanListAccess/);
	assert.match(service, /subject_group_ids/);
	assert.match(service, /s\.group_id = ANY/);
	assert.match(service, /can_manage/);
	assert.match(migration, /academic_assessment\.read\.organization_unit/);
	assert.match(migration, /unit_type = 'subject_group'/);
	assert.match(migration, /organization_permission_grants/);
});

test('academic assessment page can gate teacher access from the overview', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.svelte');

	assert.match(page, /teacherAccessEnabled/);
	assert.match(page, /toggleTeacherAccess/);
	assert.match(page, /Switch/);
	assert.match(page, /เปิดให้ครูกรอก/);
	assert.match(page, /ยังไม่เปิดให้ครูกรอกโครงสร้างคะแนน/);
});

test('academic assessment page uses one-save spreadsheet editing without expanded inline panels', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.svelte');

	assert.doesNotMatch(page, /Dialog\.Root/);
	assert.doesNotMatch(page, /Dialog\.Content/);
	assert.doesNotMatch(page, /editorOpen/);
	assert.doesNotMatch(page, /expandedPlanKey/);
	assert.doesNotMatch(page, /toggleInlineEditor/);
	assert.doesNotMatch(page, /assessment-inline-editor-row/);
	assert.doesNotMatch(page, /assessment-inline-category-grid/);
	assert.doesNotMatch(page, /assessment-inline-item-grid/);
	assert.doesNotMatch(page, /saveQuickScoreRow/);
	assert.doesNotMatch(page, /submitQuickScoreRow/);
	assert.doesNotMatch(page, /ChevronDown/);
	assert.doesNotMatch(page, /ChevronRight/);
	assert.match(page, /บันทึกการเปลี่ยนแปลง/);
});

test('academic assessment save feedback uses toast and saved status label', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.svelte');

	assert.match(page, /toast\.success\(['"]บันทึกการเปลี่ยนแปลงแล้ว['"]\)/);
	assert.doesNotMatch(page, /บันทึกคะแนนทั้งหมดแล้ว/);
	assert.match(page, /ยังไม่บันทึก/);
	assert.match(page, /\{ value: 'saved', label: 'บันทึกแล้ว' \}/);
	assert.match(page, /saved:\s*plans\.filter\(\(plan\) => plan\.status === 'saved'\)\.length/);
});

test('academic assessment plans are grouped by subject and capture exam duration', async () => {
	const api = await readProjectFile('src/lib/api/academicAssessments.ts');
	const page = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.svelte');

	assert.match(api, /classroomCount:\s*number/);
	assert.match(api, /examDurationMinutes\?:\s*number\s*\|\s*null/);
	assert.match(page, /function assessmentPlanKey\(plan: AssessmentPlanSummary\)/);
	assert.match(page, /\{#each plans as plan \(assessmentPlanKey\(plan\)\)\}/);
	assert.match(page, /ห้องเรียนที่เปิด/);
	assert.match(page, /เวลากลางภาค/);
	assert.match(page, /เวลาปลายภาค/);
	assert.match(page, /ระยะเวลากลางภาค/);
	assert.match(page, /ระยะเวลาปลายภาค/);
	assert.match(page, /examDurationMinutes/);
	assert.doesNotMatch(page, /\{#each plans as plan \(plan\.classroomCourseId\)\}/);
});
