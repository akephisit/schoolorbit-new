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

test('teaching supervision booking uses a weekly timetable grid with exact observed dates', async () => {
	const supervisionApi = await readRepoFile('frontend-school/src/lib/api/supervision.ts');
	const supervisionPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte'
	);
	const supervisionModels = await readRepoFile('backend-school/src/modules/supervision/models.rs');
	const supervisionService = await readRepoFile(
		'backend-school/src/modules/supervision/services.rs'
	);
	const migration = await readRepoFile('backend-school/migrations/008_supervision_observed_at.sql');

	assert.match(supervisionApi, /observedAt\?:\s*string\s*\|\s*null/);
	assert.match(supervisionPage, /currentBookingCycle/);
	assert.match(supervisionPage, /bookingWeekStartDate/);
	assert.match(supervisionPage, /selectedBookingDate/);
	assert.match(supervisionPage, /goToPreviousBookingWeek/);
	assert.match(supervisionPage, /goToNextBookingWeek/);
	assert.match(supervisionPage, /timetableObservedAt\(/);
	assert.match(supervisionPage, /observationForTimetableCell\(/);
	assert.match(supervisionPage, /statusLabel\(cellObservation\.status\)/);
	assert.match(supervisionPage, /observedAt:[\s\S]*manualMode[\s\S]*timetableObservedAt/);
	assert.doesNotMatch(
		supervisionPage,
		/<Select\.Root\s+type="single"\s+bind:value=\{selectedCycleId\}/
	);

	assert.match(supervisionModels, /pub\s+observed_at:\s*DateTime<Utc>/);
	assert.match(supervisionModels, /pub\s+observed_at:\s*Option<DateTime<Utc>>/);
	assert.match(supervisionService, /day_of_week_matches_observed_at/);
	assert.match(supervisionService, /validate_observed_at_in_cycle/);
	assert.match(migration, /ADD COLUMN observed_at timestamp with time zone/);
	assert.match(migration, /DROP COLUMN manual_observed_at/);
});

test('teaching supervision templates expose a read-only form preview', async () => {
	const supervisionPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte'
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
		'frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte'
	);
	const supervisionApi = await readRepoFile('frontend-school/src/lib/api/supervision.ts');
	const supervisionHandlers = await readRepoFile(
		'backend-school/src/modules/supervision/handlers.rs'
	);
	const supervisionService = await readRepoFile(
		'backend-school/src/modules/supervision/services.rs'
	);

	assert.match(supervisionApi, /SupervisionEvaluatorAvailability/);
	assert.match(supervisionApi, /conflictReason/);
	assert.match(supervisionApi, /getSupervisionEvaluatorAvailability/);
	assert.match(
		supervisionApi,
		/\/api\/supervision\/observations\/\$\{id\}\/evaluator-availability/
	);
	assert.match(supervisionPage, /requestEvaluatorIds/);
	assert.match(supervisionPage, /requestReturnComments/);
	assert.match(supervisionPage, /requestEvaluatorAvailability/);
	assert.match(supervisionPage, /handleRequestEvaluatorPickerOpen\(observation\.id,\s*open\)/);
	assert.match(supervisionPage, /loadRequestEvaluatorAvailability\(observationId\)/);
	assert.match(supervisionPage, /toggleRequestEvaluatorForRequest/);
	assert.match(supervisionPage, /selectedRequestEvaluators\(observation\.id\)/);
	assert.match(supervisionPage, /evaluator\.available/);
	assert.match(supervisionPage, /approveRequest\(observation\.id\)/);
	assert.match(supervisionPage, /evaluatorUserId:\s*evaluatorId/);
	assert.match(supervisionPage, /requestReturnComments\[observation\.id\]/);
	assert.match(supervisionPage, /observationLessonTitle\(observation\)/);
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
		'frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte'
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
		'frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte'
	);

	assert.match(supervisionPage, /evaluationDialogOpen/);
	assert.match(supervisionPage, /activeAssignedObservations/);
	assert.match(supervisionPage, /submittedAssignedObservations/);
	assert.match(supervisionPage, /currentUserEvaluator\(observation\)\?\.status !== 'submitted'/);
	assert.match(supervisionPage, /ประวัติการประเมินที่ส่งแล้ว/);
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

test('teaching supervision approval workflow skips review submission', async () => {
	const supervisionApi = await readRepoFile('frontend-school/src/lib/api/supervision.ts');
	const supervisionPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte'
	);
	const supervisionDetailPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/[id]/+page.svelte'
	);
	const supervisionHandlers = await readRepoFile(
		'backend-school/src/modules/supervision/handlers.rs'
	);
	const supervisionService = await readRepoFile(
		'backend-school/src/modules/supervision/services.rs'
	);

	assert.match(supervisionApi, /certifySupervisionObservation/);
	assert.match(supervisionApi, /\/api\/supervision\/observations\/\$\{id\}\/certify/);
	assert.doesNotMatch(supervisionApi, /submitSupervisionObservationForReview/);
	assert.doesNotMatch(supervisionApi, /publishSupervisionObservation/);
	assert.doesNotMatch(supervisionApi, /returnSupervisionObservation\(/);
	assert.match(supervisionPage, /const canReport = \$derived\([\s\S]*canManageRequests/);
	assert.match(supervisionPage, /certifiableObservations/);
	assert.match(supervisionPage, /approvableObservations/);
	assert.match(
		supervisionPage,
		/const certifiableObservations = \$derived\([\s\S]*canManageRequests[\s\S]*item\.status === 'evaluators_submitted'/
	);
	assert.match(
		supervisionPage,
		/const approvableObservations = \$derived\([\s\S]*canApprove[\s\S]*item\.status === 'approved'/
	);
	assert.doesNotMatch(supervisionPage, /certifyResult\(observation\.id\)/);
	assert.doesNotMatch(supervisionPage, /approveResult\(observation\.id\)/);
	assert.match(supervisionPage, />\s*ตรวจผล\s*</);
	assert.match(supervisionDetailPage, /async function certifyResult\(\)/);
	assert.match(supervisionDetailPage, /async function approveResult\(\)/);
	assert.match(supervisionDetailPage, />\s*รับรองผล\s*</);
	assert.match(supervisionDetailPage, />\s*อนุมัติผล\s*</);
	assert.match(supervisionPage, /รอครูรับทราบ/);
	assert.doesNotMatch(supervisionPage, /ส่งตรวจทาน/);
	assert.doesNotMatch(supervisionPage, /ส่งกลับผล/);
	assert.doesNotMatch(supervisionPage, /returnResult/);
	assert.match(supervisionDetailPage, /subject_group_certified:\s*'รับรองผล'/);
	assert.match(supervisionDetailPage, /academic_approved:\s*'อนุมัติผล'/);
	assert.doesNotMatch(supervisionDetailPage, /ส่งตรวจทาน/);
	assert.match(supervisionHandlers, /\/observations\/\{id\}\/certify/);
	assert.doesNotMatch(supervisionHandlers, /submit-review/);
	assert.match(supervisionService, /SupervisionObservationStatus::Completed/);
	assert.match(supervisionService, /status IN \('evaluators_submitted', 'under_review'\)/);
	assert.match(supervisionService, /academic_approved/);
});

test('teaching supervision observation detail supports safe edit actions', async () => {
	const supervisionApi = await readRepoFile('frontend-school/src/lib/api/supervision.ts');
	const parentPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte'
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
	assert.match(supervisionApi, /interface SupervisionAction/);
	assert.match(supervisionApi, /actions:\s*SupervisionAction\[\]/);
	assert.match(detailRoute, /_meta\s*=\s*\{\s*access:/);
	assert.doesNotMatch(detailRoute, /menu:/);
	assert.match(detailPage, /getSupervisionObservation/);
	assert.match(detailPage, /updateSupervisionObservation/);
	assert.match(detailPage, /replaceSupervisionObservationEvaluators/);
	assert.match(detailPage, /getSupervisionEvaluatorAvailability/);
	assert.match(detailPage, /getSupervisionObservationTimetableOptions/);
	assert.match(detailPage, /availableEvaluators/);
	assert.match(detailPage, /editTimetableEntries/);
	assert.match(detailPage, /selectLessonTimetableEntry/);
	assert.match(detailPage, /selectedEditTimetableEntryId/);
	assert.match(detailPage, /editTimetableObservedAt/);
	assert.match(detailPage, /timetableEntryId:\s*selectedEditTimetableEntryId/);
	assert.doesNotMatch(detailPage, /การแก้จากหน้ารายละเอียดจะบันทึกเป็นคาบกำหนดเอง/);
	assert.match(detailPage, /cancelSupervisionObservation/);
	assert.match(detailPage, /PageShell/);
	assert.match(detailPage, /LoadingButton/);
	assert.match(detailPage, /replaceObservation\(updated/);
	assert.match(detailPage, /observation\.actions/);
	assert.match(detailPage, /actionKindLabel/);
	assert.match(parentPage, /href=\{`\/staff\/academic\/supervision\/\$\{observation\.id\}`\}/);
});

test('teaching supervision detail hides scores until academic approval releases results', async () => {
	const detailPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/[id]/+page.svelte'
	);
	const supervisionHandlers = await readRepoFile(
		'backend-school/src/modules/supervision/handlers.rs'
	);
	const supervisionService = await readRepoFile(
		'backend-school/src/modules/supervision/services.rs'
	);

	assert.match(detailPage, /function observationResultsReleased/);
	assert.match(detailPage, /function observationAverageScoreLabel/);
	assert.match(detailPage, /รอหัวหน้ากลุ่มบริหารวิชาการอนุมัติผล/);
	assert.match(detailPage, /observationAverageScoreLabel\(observation\)/);
	assert.match(supervisionHandlers, /redact_observation_results_for_actor/);
	assert.match(supervisionHandlers, /redact_teacher_status_results_for_actor/);
	assert.match(supervisionService, /can_view_observation_results/);
	assert.match(supervisionService, /SupervisionObservationStatus::Published/);
	assert.match(supervisionService, /SupervisionObservationStatus::Completed/);
});

test('teaching supervision approval review shows completed rubric before approving', async () => {
	const supervisionApi = await readRepoFile('frontend-school/src/lib/api/supervision.ts');
	const supervisionPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte'
	);
	const detailPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/[id]/+page.svelte'
	);
	const supervisionHandlers = await readRepoFile(
		'backend-school/src/modules/supervision/handlers.rs'
	);
	const supervisionModels = await readRepoFile('backend-school/src/modules/supervision/models.rs');
	const supervisionService = await readRepoFile(
		'backend-school/src/modules/supervision/services.rs'
	);

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
	assert.match(detailPage, />\s*รับรองผล\s*</);
	assert.match(detailPage, />\s*อนุมัติผล\s*</);
	assert.doesNotMatch(supervisionPage, /onclick=\{\(\) => certifyResult\(observation\.id\)\}/);
	assert.doesNotMatch(supervisionPage, /onclick=\{\(\) => approveResult\(observation\.id\)\}/);
	assert.match(supervisionPage, />\s*ตรวจผล\s*</);
});

test('teaching supervision manager view exposes teacher status overview and aligned actions', async () => {
	const supervisionApi = await readRepoFile('frontend-school/src/lib/api/supervision.ts');
	const supervisionPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte'
	);
	const supervisionHandlers = await readRepoFile(
		'backend-school/src/modules/supervision/handlers.rs'
	);
	const supervisionService = await readRepoFile(
		'backend-school/src/modules/supervision/services.rs'
	);
	const supervisionPolicy = await readRepoFile(
		'backend-school/src/policies/supervision_access_policy.rs'
	);

	assert.match(supervisionApi, /SupervisionTeacherStatusRow/);
	assert.match(supervisionApi, /getSupervisionTeacherStatusOverview/);
	assert.match(
		supervisionApi,
		/\/api\/supervision\/reports\/cycles\/\$\{cycleId\}\/teacher-status/
	);
	assert.match(supervisionPage, /teacherStatusRows/);
	assert.match(supervisionPage, /loadTeacherStatusOverview/);
	assert.match(supervisionPage, /<Tabs\.Trigger value="overview"[^>]*>ภาพรวม<\/Tabs\.Trigger>/);
	assert.match(supervisionPage, /สถานะครู/);
	assert.match(supervisionPage, /<Table\.Head>กลุ่มสาระ<\/Table\.Head>/);
	assert.match(supervisionPage, /nextStepLabel/);
	assert.match(supervisionPage, /averageRating/);
	assert.match(supervisionPage, /class="flex flex-wrap items-center justify-end gap-2"/);
	assert.match(supervisionPage, /class="h-8"/);
	assert.match(supervisionHandlers, /teacher_status_overview/);
	assert.match(supervisionService, /cycle_teacher_status/);
	assert.match(supervisionService, /JOIN subject_groups sg ON sg\.id = ou\.subject_group_id/);
	assert.match(supervisionService, /ARRAY_AGG\(DISTINCT sg\.name ORDER BY sg\.name\)/);
	assert.doesNotMatch(
		supervisionService,
		/ARRAY_AGG\(ou\.name ORDER BY om\.is_primary DESC, ou\.name\)/
	);
	assert.match(
		supervisionPolicy,
		/require_observation_management_access[\s\S]*\.await[\s\S]*return Ok\(\(\)\)/
	);
});
