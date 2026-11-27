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
			'src/lib/components/learning-delivery/OfferingCreateDialog.svelte',
			'src/lib/components/learning-delivery/OfferingCurriculumPreview.svelte',
			'src/routes/(app)/staff/academic/delivery/+page.svelte'
		].map(readProjectFile)
	);
	const workflow = sources.join('\n');

	assert.doesNotMatch(workflow, /ชุดการเรียน/);
	assert.match(workflow, /รายการเปิดสอน/);
});

test('delivery guidance uses the shared local prerequisite notice', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/delivery/+page.svelte');

	assert.match(page, /เลือกปีการศึกษาและภาคเรียนก่อน/);
	assert.match(page, /DeliveryMissingRequirements/);
	assert.match(page, /prepare/);
	assert.doesNotMatch(page, /globalReadiness|completionPercent/);
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
		'src/routes/(app)/staff/academic/supervision/mine/+page.svelte'
	);
	const supervisionWorkspace = await readProjectFile(
		'src/lib/components/supervision/SupervisionWorkspace.svelte'
	);
	const gradebookPage = await readProjectFile(
		'src/routes/(app)/staff/academic/gradebook/+page.svelte'
	);
	const resultsPage = await readProjectFile('src/routes/(app)/staff/academic/results/+page.svelte');
	const resultLocksPage = await readProjectFile(
		'src/routes/(app)/staff/academic/result-locks/+page.svelte'
	);
	const resultLocksQueue = await readProjectFile(
		'src/lib/components/academic/results/ResultLockQueue.svelte'
	);
	const resultCorrectionsPage = await readProjectFile(
		'src/routes/(app)/staff/academic/result-corrections/+page.svelte'
	);
	const allPages = [
		activityPage,
		assessmentPage,
		timetablePage,
		examListPage,
		examDetailPage,
		supervisionPage,
		gradebookPage,
		resultsPage,
		resultLocksPage,
		resultCorrectionsPage
	].join('\n');

	assert.match(activityPage, /\/staff\/academic\/delivery\?kind=activity/);
	assert.match(assessmentPage, /AcademicPrerequisiteNotice/);
	assert.match(assessmentPage, /\/staff\/academic\/delivery/);
	assert.match(timetablePage, /AcademicPrerequisiteNotice/);
	assert.match(timetablePage, /\/staff\/academic\/core#bell-schedules/);
	assert.match(timetablePage, /\/staff\/facility\/buildings/);
	assert.match(examListPage, /\/staff\/academic\/assessments/);
	assert.match(examDetailPage, /\/staff\/academic\/assessments/);
	assert.doesNotMatch(examDetailPage, /deliveryTargets|แก้ข้อมูลต้นทาง:/);
	assert.match(
		supervisionPage,
		/<SupervisionWorkspace\s+section="mine"\s+routeData=\{data\}\s*\/>/
	);
	assert.doesNotMatch(
		supervisionPage,
		/AcademicPrerequisiteNotice|bookingTermPrerequisite|\{#if\s+!academicTermId/
	);
	for (const field of ['academicYearId', 'academicTermId']) {
		assert.match(
			supervisionWorkspace,
			new RegExp(`${field} = \\$derived\\(\\s*routeData \\? routeData\\.${field}`)
		);
	}
	assert.match(
		supervisionWorkspace,
		/\{#if cyclesLoaded && currentBookingCycle && academicYearId && academicTermId\}/
	);
	assert.match(
		supervisionWorkspace,
		/<SupervisionLessonPicker[\s\S]*?\{academicYearId\}[\s\S]*?\{academicTermId\}/
	);
	assert.match(
		supervisionWorkspace,
		/createBookingRequest[\s\S]*?if \(!canRequest \|\| !currentBookingCycle \|\| !academicTermId\)/
	);
	assert.match(
		supervisionWorkspace,
		/\{#snippet referenceErrors[\s\S]*?onaction=\{retryManagementCycles\}[\s\S]*?onaction=\{retryManagementTemplates\}[\s\S]*?\{\/snippet\}/
	);
	assert.match(
		supervisionWorkspace,
		/id="supervision-booking-panel"[\s\S]*?\{@render referenceErrors\(false\)\}[\s\S]*?<SupervisionLessonPicker/
	);
	assert.match(
		supervisionWorkspace,
		/<details[^>]*>[\s\S]*?\{@render referenceErrors\(true\)\}[\s\S]*?\{@render observationFactItems\(observationReferenceFacts\(observation\)\)\}/
	);
	assert.match(supervisionWorkspace, /title="ยังไม่มีรอบนิเทศที่เปิดให้จองในขณะนี้"/);
	assert.match(supervisionWorkspace, /เมื่อผู้ดูแลเปิดรับคำขอ คุณจะเลือกคาบจากตารางสอนได้ที่นี่/);
	assert.match(gradebookPage, /AcademicPrerequisiteNotice/);
	assert.match(gradebookPage, /\/staff\/academic\/delivery/);
	assert.match(gradebookPage, /\/staff\/academic\/assessments/);
	assert.match(resultsPage, /AcademicPrerequisiteNotice/);
	assert.match(resultsPage, /\/staff\/academic\/gradebook/);
	assert.match(resultLocksQueue, /AcademicPrerequisiteNotice/);
	assert.match(resultLocksPage, /\/staff\/academic\/results/);
	assert.match(resultCorrectionsPage, /AcademicPrerequisiteNotice/);
	assert.match(resultCorrectionsPage, /\/staff\/academic\/result-locks/);
	assert.doesNotMatch(allPages, /readinessScore|completionPercent|ศูนย์เตรียมงานวิชาการ/);
	assert.doesNotMatch(
		allPages,
		/getLearningDeliveryManagementOptions|getCurriculumManagementOptions|getCurriculumProgramWorkspace/
	);
});
