import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '../..');

async function readProjectFile(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
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
});

test('academic assessment route exposes overview, downloads, and nested score item editing', async () => {
	const meta = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.ts');
	const page = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.svelte');

	assert.match(meta, /title:\s*['"]โครงสร้างคะแนน['"]/);
	assert.match(meta, /permission:\s*PERMISSION_MODULES\.ACADEMIC_ASSESSMENT/);
	assert.match(meta, /group:\s*['"]academic['"]/);

	assert.match(page, /PageShell/);
	assert.match(page, /Download/);
	assert.match(page, /exportAssessmentReport/);
	assert.match(page, /openPlanEditor/);
	assert.match(page, /addCategory/);
	assert.match(page, /addItem/);
	assert.match(page, /removeItem/);
	assert.match(page, /examModeOptions/);
	assert.match(page, /outside_timetable/);
	assert.match(page, /allocationStatusLabel/);
});

test('academic assessment page can gate teacher access from the overview', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.svelte');

	assert.match(page, /teacherAccessEnabled/);
	assert.match(page, /toggleTeacherAccess/);
	assert.match(page, /Switch/);
	assert.match(page, /เปิดให้ครูกรอก/);
	assert.match(page, /ยังไม่เปิดให้ครูกรอกโครงสร้างคะแนน/);
});
