import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = join(dirname(fileURLToPath(import.meta.url)), '../..');

async function readProjectFile(relativePath) {
	return readFile(join(projectRoot, relativePath), 'utf8');
}

test('academic prerequisites stay page-local and action-specific', async () => {
	const model = await readProjectFile('src/lib/components/academic-workflow/prerequisite.ts');
	const notice = await readProjectFile(
		'src/lib/components/academic-workflow/AcademicPrerequisiteNotice.svelte'
	);

	assert.match(model, /status: 'missing' \| 'warning'/);
	assert.match(model, /href\?: string/);
	assert.match(notice, /ทางไปต่อ/);
	assert.match(notice, /prerequisite\.actionLabel && prerequisite\.href/);
	assert.doesNotMatch(model, /global|completionPercent|readinessScore/i);
	assert.doesNotMatch(notice, /onMount|fetch\(|getAcademicContextStore/);
});

test('learning delivery no longer calls offerings ชุดการเรียน', async () => {
	const sources = await Promise.all(
		[
			'src/lib/api/learning-delivery.ts',
			'src/lib/api/academicAssessments.ts',
			'src/lib/components/learning-delivery/LearningOfferingEditor.svelte',
			'src/lib/components/learning-delivery/CurriculumOfferingPreview.svelte',
			'src/routes/(app)/staff/academic/delivery/+page.svelte'
		].map(readProjectFile)
	);
	const workflow = sources.join('\n');

	assert.doesNotMatch(workflow, /ชุดการเรียน/);
	assert.match(workflow, /รายการเปิดสอน/);
});

test('delivery guidance uses the shared local prerequisite notice', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/delivery/+page.svelte');

	assert.match(page, /AcademicPrerequisiteNotice/);
	assert.match(page, /missingTermPrerequisite/);
	assert.match(page, /noOfferingPrerequisite/);
});

test('dependent academic pages provide local next actions without a global readiness center', async () => {
	const activityPage = await readProjectFile(
		'src/routes/(app)/staff/academic/catalog/activities/+page.svelte'
	);
	const assessmentPage = await readProjectFile(
		'src/routes/(app)/staff/academic/assessments/+page.svelte'
	);
	const timetablePage = await readProjectFile(
		'src/routes/(app)/staff/academic/timetable/+page.svelte'
	);
	const examListPage = await readProjectFile(
		'src/routes/(app)/staff/academic/exam-schedules/+page.svelte'
	);
	const examDetailPage = await readProjectFile(
		'src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'
	);
	const supervisionPage = await readProjectFile(
		'src/routes/(app)/staff/academic/supervision/+page.svelte'
	);
	const allPages = [
		activityPage,
		assessmentPage,
		timetablePage,
		examListPage,
		examDetailPage,
		supervisionPage
	].join('\n');

	assert.match(activityPage, /\/staff\/academic\/delivery\?kind=activity/);
	assert.match(assessmentPage, /AcademicPrerequisiteNotice/);
	assert.match(assessmentPage, /\/staff\/academic\/delivery/);
	assert.match(timetablePage, /AcademicPrerequisiteNotice/);
	assert.match(timetablePage, /\/staff\/academic\/core#bell-schedules/);
	assert.match(timetablePage, /\/staff\/facility\/buildings/);
	assert.match(examListPage, /\/staff\/academic\/delivery/);
	assert.match(examDetailPage, /\/staff\/academic\/delivery\/\$\{/);
	assert.match(supervisionPage, /AcademicPrerequisiteNotice/);
	assert.doesNotMatch(allPages, /readinessScore|completionPercent|ศูนย์เตรียมงานวิชาการ/);
	assert.doesNotMatch(
		allPages,
		/getLearningDeliveryManagementOptions|getCurriculumManagementOptions|getCurriculumProgramWorkspace/
	);
});
