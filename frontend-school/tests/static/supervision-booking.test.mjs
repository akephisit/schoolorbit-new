import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../../..');

async function readRepoFile(relativePath) {
	return readFile(path.join(repoRoot, relativePath), 'utf8');
}

const supervisionServiceFiles = [
	'backend-school/crates/school-supervision/src/services.rs',
	'backend-school/crates/school-supervision/src/services/cycles.rs',
	'backend-school/crates/school-supervision/src/services/evaluations.rs',
	'backend-school/crates/school-supervision/src/services/observations.rs',
	'backend-school/crates/school-supervision/src/services/reviews_and_reports.rs',
	'backend-school/crates/school-supervision/src/services/response_visibility.rs',
	'backend-school/crates/school-supervision/src/services/shared.rs',
	'backend-school/crates/school-supervision/src/services/templates.rs'
];

async function readSupervisionServices() {
	return (await Promise.all(supervisionServiceFiles.map(readRepoFile))).join('\n');
}

test('teaching supervision names canonical timetable block-group references end to end', async () => {
	const supervisionModels = await readRepoFile(
		'backend-school/crates/school-supervision/src/models.rs'
	);
	const supervisionServices = await readSupervisionServices();
	const generatedSchoolApi = await readRepoFile(
		'frontend-school/src/lib/api/generated/school-api.ts'
	);

	assert.match(supervisionModels, /timetable_block_group_id/);
	assert.match(supervisionServices, /timetable_block_group_id/);
	assert.match(generatedSchoolApi, /timetableBlockGroupId/);
	assert.doesNotMatch(supervisionModels, /timetable_entry_id/);
	assert.doesNotMatch(supervisionServices, /timetable_entry_id/);
	assert.doesNotMatch(generatedSchoolApi, /timetableEntryId/);
});

test('teaching supervision booking uses canonical dated options and an ordered range without manual input', async () => {
	const api = await readRepoFile('frontend-school/src/lib/api/supervision.ts');
	const page = await readRepoFile(
		'frontend-school/src/lib/components/supervision/SupervisionWorkspace.svelte'
	);
	const picker = await readRepoFile(
		'frontend-school/src/lib/components/supervision/SupervisionLessonPicker.svelte'
	);
	const range = await readRepoFile('frontend-school/src/lib/supervision/booking-range.ts');
	assert.match(api, /operations\['getSupervisionBookingOptions'\]/);
	assert.match(api, /\/api\/supervision\/booking-options/);
	assert.match(page, /<SupervisionLessonPicker/);
	assert.match(
		page,
		/observations=\{myObservations.filter\([\s\S]*item.cycleId === currentBookingCycle.id/
	);
	assert.match(picker, /weekStartDate/);
	assert.match(picker, /changeWeek\(-1\)/);
	assert.match(picker, /changeWeek\(1\)/);
	assert.match(picker, /timetableBlockGroupIds: ordered.map/);
	assert.match(picker, /observedDate: ordered\[0\].observedDate/);
	assert.match(picker, /expectedSourceDigest: selectionDigest/);
	assert.match(picker, /bookedObservationFor/);
	assert.match(range, /observation.periods.some/);
	assert.match(range, /item.homeroomId !== first.homeroomId/);
	assert.doesNotMatch(page, /manualMode|manualLessonDate|getMyTimetable|combineLocalDateTime/);
	assert.doesNotMatch(picker, /manualLesson:|type="time"|toISOString\(\)/);
	assert.doesNotMatch(page, /bookingOpensAt|bookingClosesAt|cycle.startsAt|cycle.endsAt/);
});

test('teaching supervision booking relies on required route context while management remains year-wide', async () => {
	const root = 'frontend-school/src/routes/(app)/staff/academic/supervision';
	const routePage = await readRepoFile(`${root}/+page.svelte`);
	const routeLoader = await readRepoFile(`${root}/+page.ts`);
	assert.match(routeLoader, /academicContext:\s*'term_required'/);
	assert.match(routePage, /<SupervisionWorkspace section="mine"/);
	assert.doesNotMatch(routePage, /AcademicPrerequisiteNotice|bookingTermPrerequisite/);
	for (const child of ['evaluate', 'requests', 'cycles', 'templates', 'overview', '[id]']) {
		const loader = await readRepoFile(`${root}/${child}/+page.ts`);
		assert.match(loader, /academicContext:\s*'term_optional'/, child);
	}
});

test('teaching supervision templates expose a read-only form preview', async () => {
	const supervisionPage = await readRepoFile(
		'frontend-school/src/lib/components/supervision/SupervisionWorkspace.svelte'
	);

	assert.match(supervisionPage, /previewTemplateDialogOpen/);
	assert.match(supervisionPage, /openTemplatePreviewDialog\(template\)/);
	assert.match(supervisionPage, />\s*ดูตัวอย่าง\s*</);
	assert.match(supervisionPage, /<Dialog\.Title>ตัวอย่างแบบประเมินนิเทศ<\/Dialog\.Title>/);
	assert.match(supervisionPage, /templateRatingColumns\(previewTemplate\)/);
	assert.match(supervisionPage, /\{#each previewTemplate\.sections as section/);
	assert.match(supervisionPage, /\{#each section\.items as item/);
	assert.match(supervisionPage, /aria-label=\{`ช่องคะแนน \$\{score\}/);
	assert.match(supervisionPage, /readonly/i);
});

test('teaching supervision request approval renders request rows with multiple evaluators', async () => {
	const supervisionPage = await readRepoFile(
		'frontend-school/src/lib/components/supervision/SupervisionWorkspace.svelte'
	);
	const review = await readRepoFile(
		'frontend-school/src/lib/components/supervision/SupervisionRequestReview.svelte'
	);
	const requestCard = await readRepoFile(
		'frontend-school/src/lib/components/supervision/SupervisionRequestCard.svelte'
	);
	const controller = await readRepoFile(
		'frontend-school/src/lib/supervision/request-review.svelte.ts'
	);
	const detail = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/[id]/+page.svelte'
	);
	const supervisionApi = await readRepoFile('frontend-school/src/lib/api/supervision.ts');
	const generatedSchoolApi = await readRepoFile(
		'frontend-school/src/lib/api/generated/school-api.ts'
	);
	const supervisionHandlers = await readRepoFile(
		'backend-school/crates/school-supervision-http/src/handlers.rs'
	);
	const supervisionService = await readSupervisionServices();

	assert.match(supervisionApi, /SupervisionEvaluatorAvailability/);
	assert.match(
		generatedSchoolApi,
		/SupervisionEvaluatorAvailability:\s*\{[\s\S]*?conflictReason\?: string \| null;/
	);
	assert.match(supervisionApi, /getSupervisionEvaluatorAvailability/);
	assert.match(
		supervisionApi,
		/\/api\/supervision\/observations\/\$\{id\}\/evaluator-availability/
	);
	assert.equal([...supervisionPage.matchAll(/<SupervisionRequestReview\b/g)].length, 2);
	assert.match(detail, /<SupervisionRequestReview/);
	assert.match(review, /\{#each selected as evaluator/);
	assert.match(review, /if \(pickerOpen\) void review\.loadAvailability\(id\)/);
	assert.match(review, /review\.toggleEvaluator\(id, evaluator\)/);
	assert.match(review, /review\.setComment\(id, event\.currentTarget\.value\)/);
	assert.match(review, /review\.submit\(id, 'approve-request'\)/);
	assert.match(controller, /getSupervisionEvaluatorAvailability\(id, \{ signal \}\)/);
	assert.match(controller, /state\?\.evaluatorIds \?\? observation\.evaluators\.map/);
	assert.match(controller, /evaluatorUserId:\s*id,\s*isRequired,/);
	assert.match(controller, /returnSupervisionObservationRequest\(id, \{ comment: drafts\[id\]/);
	assert.match(requestCard, /savedLessonLabels\(observation\)/);
	assert.match(supervisionHandlers, /evaluator_availability/);
	assert.match(supervisionService, /validate_evaluator_availability_for_observation/);
	assert.doesNotMatch(
		supervisionPage,
		/<Select\.Root\s+type="single"\s+bind:value=\{approvalObservationId\}/
	);
	assert.doesNotMatch(supervisionPage, /approvalEvaluatorId/);
});

test('teaching supervision own and assigned lists show complete lesson and evaluator context', async () => {
	const supervisionPage = await readRepoFile(
		'frontend-school/src/lib/components/supervision/SupervisionWorkspace.svelte'
	);

	assert.match(
		supervisionPage,
		/function observationSubjectLabel\(observation: SupervisionObservation\)/
	);
	assert.match(
		supervisionPage,
		/function observationClassroomLabel\(observation: SupervisionObservation\)/
	);
	assert.match(
		supervisionPage,
		/function observationPeriodLabel\(observation: SupervisionObservation\)/
	);
	assert.match(
		supervisionPage,
		/function observationRoomLabel\(observation: SupervisionObservation\)/
	);
	assert.match(
		supervisionPage,
		/function observationEvaluatorNames\(observation: SupervisionObservation\)/
	);
	assert.match(
		supervisionPage,
		/function observationDetailGrid\(observation: SupervisionObservation\)/
	);
	assert.match(supervisionPage, /ผู้นิเทศ/);
	assert.match(supervisionPage, /นิเทศใคร/);
	assert.match(supervisionPage, /เปิดแบบประเมิน/);
	assert.match(supervisionPage, /observationDetailGrid\(observation\)/);
	assert.match(supervisionPage, /observationEvaluatorNames\(observation\)/);
	assert.match(supervisionPage, /data-supervision-own-list="cards"/);
	assert.match(supervisionPage, /data-supervision-assigned-list="cards"/);
});

test('teaching supervision evaluation uses a dialog workflow', async () => {
	const supervisionApi = await readRepoFile('frontend-school/src/lib/api/supervision.ts');
	const supervisionPage = await readRepoFile(
		'frontend-school/src/lib/components/supervision/SupervisionWorkspace.svelte'
	);

	assert.match(supervisionPage, /evaluationDialogOpen/);
	assert.match(supervisionPage, /activeAssignedObservations/);
	assert.match(supervisionPage, /submittedAssignedObservations/);
	assert.match(supervisionPage, /evaluator\.status !== 'submitted'/);
	assert.match(supervisionPage, /ผลประเมินที่ส่งแล้ว/);
	assert.match(supervisionPage, /setEvaluationDialogOpen\(open: boolean\)/);
	assert.match(supervisionPage, /clearEvaluationDraft\(\)/);
	assert.match(supervisionPage, /<Dialog\.Root bind:open=\{evaluationDialogOpen\}/);
	assert.match(supervisionPage, /<Dialog\.Title>ทำแบบประเมินนิเทศ<\/Dialog\.Title>/);
	assert.match(supervisionPage, /ตอบแล้ว/);
	assert.match(supervisionPage, /progress\.totalScore/);
	assert.match(supervisionPage, /progress\.maxScore/);
	assert.match(supervisionPage, /progress\.qualityLabel/);
	assert.match(supervisionPage, /selectedEvaluationDraftSummary\.totalScore/);
	assert.match(supervisionPage, /clearEvaluationDraft\(\);[\s\S]*toast\.success/);
	assert.doesNotMatch(supervisionPage, /saveMySupervisionEvaluation/);
	assert.doesNotMatch(supervisionPage, /saveEvaluation\(false\)/);
	assert.doesNotMatch(supervisionPage, />\s*บันทึกร่าง\s*</);
	assert.doesNotMatch(supervisionApi, /saveMySupervisionEvaluation/);
	assert.doesNotMatch(supervisionPage, /กำลังเปิดแบบประเมิน/);
});

test('teaching supervision publishes after evaluation and retains request approval and teacher acknowledgment', async () => {
	const supervisionApi = await readRepoFile('frontend-school/src/lib/api/supervision.ts');
	const workspace = await readRepoFile(
		'frontend-school/src/lib/components/supervision/SupervisionWorkspace.svelte'
	);
	const detailPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/[id]/+page.svelte'
	);
	const handlers = await readRepoFile(
		'backend-school/crates/school-supervision-http/src/handlers.rs'
	);

	assert.match(supervisionApi, /\/api\/supervision\/observations\/\$\{id\}\/approve-request/);
	assert.match(
		supervisionApi,
		/\/api\/supervision\/observations\/\$\{id\}\/evaluations\/me\/submit/
	);
	assert.match(supervisionApi, /\/api\/supervision\/observations\/\$\{id\}\/acknowledge/);
	assert.doesNotMatch(
		supervisionApi,
		/\/observations\/\$\{id\}\/(?:certify|approve|submit-review)`/
	);
	assert.doesNotMatch(handlers, /\/observations\/\{id\}\/(?:certify|approve|submit-review)"/);
	assert.doesNotMatch(
		workspace + detailPage,
		/รับรองผล|อนุมัติผล|evaluators_submitted|under_review/
	);
	assert.match(workspace, /เมื่อผู้ประเมินที่จำเป็นส่งผลครบ ระบบจะแจ้งให้ครูรับทราบผล/);
	assert.match(workspace, /{#if observation\.status === 'published'}/);
	assert.doesNotMatch(detailPage, /actionKindLabel|<Card\.Title>ประวัติ<\/Card\.Title>/);
});

test('teaching supervision observation detail supports safe edit actions', async () => {
	const supervisionApi = await readRepoFile('frontend-school/src/lib/api/supervision.ts');
	const parentPage = await readRepoFile(
		'frontend-school/src/lib/components/supervision/SupervisionWorkspace.svelte'
	);
	const detailRoute = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/[id]/+page.ts'
	);
	const detailPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/[id]/+page.svelte'
	);

	assert.match(supervisionApi, /updateSupervisionObservation/);
	assert.match(supervisionApi, /replaceSupervisionObservationEvaluators/);
	assert.match(supervisionApi, /getSupervisionEvaluatorAvailability/);
	assert.match(supervisionApi, /getSupervisionObservationTimetableOptions/);
	assert.match(supervisionApi, /cancelSupervisionObservation/);
	assert.match(supervisionApi, /SupervisionAction = Schemas\['SupervisionAction'\]/);
	assert.match(
		detailRoute,
		/_meta\s*=\s*\{\s*academicContext:\s*'term_optional' as const,\s*access:/
	);
	assert.doesNotMatch(detailRoute, /menu:/);
	assert.match(detailPage, /getSupervisionObservation/);
	assert.match(detailPage, /updateSupervisionObservation/);
	assert.match(detailPage, /replaceSupervisionObservationEvaluators/);
	assert.match(detailPage, /getSupervisionEvaluatorAvailability/);
	assert.match(detailPage, /availableEvaluators/);
	assert.match(detailPage, /<SupervisionLessonPicker/);
	assert.match(detailPage, /<SupervisionSavedPeriods/);
	assert.match(detailPage, /saveLessonEdit\(selection: SupervisionLessonSelection\)/);
	assert.doesNotMatch(detailPage, /lessonForm|supervision-manual-|manualLesson:/);
	assert.doesNotMatch(detailPage, /การแก้จากหน้ารายละเอียดจะบันทึกเป็นคาบกำหนดเอง/);
	assert.match(detailPage, /cancelSupervisionObservation/);
	assert.match(detailPage, /PageShell/);
	assert.match(detailPage, /LoadingButton/);
	assert.match(detailPage, /replaceObservation\(updated/);
	assert.match(detailPage, /currentReturnReason\(observation\)/);
	assert.doesNotMatch(detailPage, /actionKindLabel|<Card\.Title>ประวัติ<\/Card\.Title>/);
	assert.match(parentPage, /href=\{observationDetailHref\(observation\)\}/);
});

test('teaching supervision detail protects scores until results are published', async () => {
	const detailPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/[id]/+page.svelte'
	);
	const supervisionHandlers = await readRepoFile(
		'backend-school/crates/school-supervision-http/src/handlers.rs'
	);
	const supervisionService = await readSupervisionServices();

	assert.match(detailPage, /function observationResultsReleased/);
	assert.match(detailPage, /function observationAverageScoreLabel/);
	assert.match(detailPage, /รอเผยแพร่ผล/);
	assert.match(detailPage, /observationAverageScoreLabel\(observation\)/);
	assert.match(supervisionHandlers, /services::redacted_observation_for_actor/);
	assert.match(supervisionHandlers, /services::redact_observations_results_for_actor/);
	assert.match(supervisionService, /fn redact_observation_results_for_actor/);
	assert.match(supervisionService, /managed_user_ids\.contains\(&observation\.observed_user_id\)/);
	assert.doesNotMatch(supervisionHandlers, /fn redact_observation_results_for_actor/);
	assert.match(supervisionHandlers, /redact_teacher_status_results_for_actor/);
	assert.match(supervisionService, /can_view_observation_results/);
	assert.match(supervisionService, /SupervisionObservationStatus::Published/);
	assert.match(supervisionService, /SupervisionObservationStatus::Completed/);
});

test('teaching supervision published results retain the read-only rubric and evaluator breakdown', async () => {
	const supervisionApi = await readRepoFile('frontend-school/src/lib/api/supervision.ts');
	const detailPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/[id]/+page.svelte'
	);
	const supervisionHandlers = await readRepoFile(
		'backend-school/crates/school-supervision-http/src/handlers.rs'
	);
	const supervisionModels = await readRepoFile(
		'backend-school/crates/school-supervision/src/models.rs'
	);
	const supervisionService = await readSupervisionServices();

	assert.match(supervisionApi, /SupervisionObservationReview/);
	assert.match(supervisionApi, /SupervisionReviewEvaluatorResult/);
	assert.match(supervisionApi, /getSupervisionObservationReview/);
	assert.match(supervisionApi, /\/api\/supervision\/observations\/\$\{id\}\/review/);
	assert.match(supervisionHandlers, /get_observation_review/);
	assert.match(supervisionHandlers, /\/observations\/\{id\}\/review/);
	assert.match(supervisionModels, /pub struct SupervisionObservationReview/);
	assert.match(supervisionModels, /pub struct SupervisionReviewEvaluatorResult/);
	assert.match(supervisionModels, /pub struct SupervisionReviewItemSummary/);
	assert.match(supervisionService, /pub async fn get_observation_review/);
	assert.match(supervisionService, /load_observation_review_responses/);
	assert.match(detailPage, /review = \$state<SupervisionObservationReview \| null>\(null\)/);
	assert.match(detailPage, /loadReviewDetail/);
	assert.match(detailPage, /reviewAverageScoreLabel/);
	assert.match(detailPage, /selectedReviewEvaluatorId/);
	assert.match(detailPage, /reviewItemRatingFor/);
	assert.match(detailPage, /data-supervision-review-rubric="readonly"/);
	assert.match(detailPage, /observationResultsReleased\(observation\.status\)/);
	assert.match(detailPage, /ดูผลประเมินรายข้อ/);
});

test('supervision keeps separate workflow routes with an in-place requests calendar', async () => {
	const supervisionWorkspace = await readRepoFile(
		'frontend-school/src/lib/components/supervision/SupervisionWorkspace.svelte'
	);
	const routeAccess = await readRepoFile('frontend-school/src/lib/auth/route-access.ts');
	const routeFiles = {
		base: await readRepoFile(
			'frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte'
		),
		evaluate: await readRepoFile(
			'frontend-school/src/routes/(app)/staff/academic/supervision/evaluate/+page.svelte'
		),
		requests: await readRepoFile(
			'frontend-school/src/routes/(app)/staff/academic/supervision/requests/+page.svelte'
		),
		overview: await readRepoFile(
			'frontend-school/src/routes/(app)/staff/academic/supervision/overview/+page.svelte'
		),
		cycles: await readRepoFile(
			'frontend-school/src/routes/(app)/staff/academic/supervision/cycles/+page.svelte'
		),
		templates: await readRepoFile(
			'frontend-school/src/routes/(app)/staff/academic/supervision/templates/+page.svelte'
		)
	};
	const routeMetaFiles = {
		evaluate: await readRepoFile(
			'frontend-school/src/routes/(app)/staff/academic/supervision/evaluate/+page.ts'
		),
		requests: await readRepoFile(
			'frontend-school/src/routes/(app)/staff/academic/supervision/requests/+page.ts'
		),
		overview: await readRepoFile(
			'frontend-school/src/routes/(app)/staff/academic/supervision/overview/+page.ts'
		),
		cycles: await readRepoFile(
			'frontend-school/src/routes/(app)/staff/academic/supervision/cycles/+page.ts'
		),
		templates: await readRepoFile(
			'frontend-school/src/routes/(app)/staff/academic/supervision/templates/+page.ts'
		)
	};

	const requests = supervisionWorkspace.match(
		/\{:else if section === 'requests'\}([\s\S]*?)\{:else if section === 'evaluate'\}/
	)?.[1];
	const evaluate = supervisionWorkspace.match(
		/\{:else if section === 'evaluate'\}([\s\S]*?)\{:else if section === 'cycles'\}/
	)?.[1];
	assert.ok(requests, 'request presentation stays in the requests route section');
	assert.ok(evaluate, 'evaluation presentation stays in the evaluate route section');
	assert.equal([...supervisionWorkspace.matchAll(/<Tabs\.Root\b/g)].length, 2);
	assert.match(requests, /<Tabs\.Root\s+bind:value=\{queueView\}/);
	assert.match(requests, /<SupervisionBookingCalendar\s+\{observations\}/);
	assert.match(evaluate, /<Tabs\.Root\s+bind:value=\{queueView\}/);
	assert.match(
		evaluate,
		/<SupervisionBookingCalendar\s+observations=\{evaluationCalendarObservations\}/
	);
	assert.doesNotMatch(supervisionWorkspace, /bind:value=\{activeTab\}/);
	assert.match(supervisionWorkspace, /export type SupervisionWorkspaceSection/);
	assert.match(supervisionWorkspace, /section:/);
	assert.match(supervisionWorkspace, /function sectionRoute/);
	assert.match(supervisionWorkspace, /routeData\.templates/);
	assert.doesNotMatch(supervisionWorkspace, /listSupervisionTemplates/);
	assert.match(supervisionWorkspace, /shouldLoadObservations/);
	assert.match(supervisionWorkspace, /getAcademicContextStore/);
	assert.match(supervisionWorkspace, /academicTermId/);
	assert.match(routeAccess, /permission\?: RoutePermission \| RoutePermission\[\]/);
	assert.match(routeAccess, /Array\.isArray\(requiredPermission\)/);

	for (const [section, source] of Object.entries(routeFiles)) {
		assert.match(source, /SupervisionWorkspace/);
		assert.match(source, new RegExp(`section="${section === 'base' ? 'mine' : section}"`));
	}

	for (const [section, source] of Object.entries(routeMetaFiles)) {
		assert.match(source, /_meta/);
		assert.match(source, /access/);
		assert.doesNotMatch(source, /menu:/);
		assert.match(source, /PERMISSIONS\./, `${section} route should use exact permission access`);
	}
});

test('teaching supervision manager view exposes teacher status overview and aligned actions', async () => {
	const supervisionApi = await readRepoFile('frontend-school/src/lib/api/supervision.ts');
	const supervisionWorkspace = await readRepoFile(
		'frontend-school/src/lib/components/supervision/SupervisionWorkspace.svelte'
	);
	const supervisionHandlers = await readRepoFile(
		'backend-school/crates/school-supervision-http/src/handlers.rs'
	);
	const supervisionService = await readSupervisionServices();
	const supervisionPolicy = await readRepoFile(
		'backend-school/crates/school-supervision/src/access_policy.rs'
	);

	assert.match(supervisionApi, /SupervisionTeacherStatusRow/);
	assert.match(supervisionApi, /getSupervisionTeacherStatusOverview/);
	assert.match(
		supervisionApi,
		/\/api\/supervision\/reports\/cycles\/\$\{cycleId\}\/teacher-status/
	);
	assert.match(supervisionWorkspace, /teacherStatusRows/);
	assert.match(supervisionWorkspace, /loadTeacherStatusOverview/);
	assert.match(supervisionWorkspace, /สถานะครู/);
	assert.match(supervisionWorkspace, /<Table\.Head>กลุ่มสาระ<\/Table\.Head>/);
	assert.match(supervisionWorkspace, /nextStepLabel/);
	assert.match(supervisionWorkspace, /averageRating/);
	assert.match(supervisionWorkspace, /data-testid="supervision-progress"/);
	assert.match(supervisionWorkspace, /progress\.publishedCount/);
	assert.match(supervisionWorkspace, /progress\.completedCount/);
	assert.match(supervisionHandlers, /teacher_status_overview/);
	assert.match(supervisionService, /cycle_teacher_status/);
	assert.match(supervisionService, /JOIN subject_groups sg ON sg\.id = ou\.subject_group_id/);
	assert.match(supervisionService, /ARRAY_AGG\(DISTINCT sg\.name_th ORDER BY sg\.name_th\)/);
	assert.doesNotMatch(supervisionService, /sg\.name\b/);
	assert.doesNotMatch(
		supervisionService,
		/ARRAY_AGG\(ou\.name ORDER BY om\.is_primary DESC, ou\.name\)/
	);
	assert.match(
		supervisionPolicy,
		/require_observation_management_access[\s\S]*\.await[\s\S]*return Ok\(\(\)\)/
	);
});
