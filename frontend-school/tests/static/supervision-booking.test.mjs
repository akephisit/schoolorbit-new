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

	assert.match(supervisionPage, /requestEvaluatorIds/);
	assert.match(supervisionPage, /requestReturnComments/);
	assert.match(supervisionPage, /toggleRequestEvaluatorForRequest/);
	assert.match(supervisionPage, /selectedRequestEvaluators\(observation\.id\)/);
	assert.match(supervisionPage, /approveRequest\(observation\.id\)/);
	assert.match(supervisionPage, /evaluatorUserId:\s*evaluatorId/);
	assert.match(supervisionPage, /requestReturnComments\[observation\.id\]/);
	assert.match(supervisionPage, /observationLessonTitle\(observation\)/);
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
	assert.doesNotMatch(supervisionPage, /กำลังเปิดแบบประเมิน/);
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
	assert.match(supervisionApi, /cancelSupervisionObservation/);
	assert.match(supervisionApi, /interface SupervisionAction/);
	assert.match(supervisionApi, /actions:\s*SupervisionAction\[\]/);
	assert.match(detailRoute, /_meta\s*=\s*\{\s*access:/);
	assert.doesNotMatch(detailRoute, /menu:/);
	assert.match(detailPage, /getSupervisionObservation/);
	assert.match(detailPage, /updateSupervisionObservation/);
	assert.match(detailPage, /replaceSupervisionObservationEvaluators/);
	assert.match(detailPage, /cancelSupervisionObservation/);
	assert.match(detailPage, /PageShell/);
	assert.match(detailPage, /LoadingButton/);
	assert.match(detailPage, /replaceObservation\(updated/);
	assert.match(detailPage, /observation\.actions/);
	assert.match(detailPage, /actionKindLabel/);
	assert.match(parentPage, /href=\{`\/staff\/academic\/supervision\/\$\{observation\.id\}`\}/);
});
