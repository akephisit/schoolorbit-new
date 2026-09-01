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

test('academic assessment permissions remain generated for teachers and academic office', async () => {
	const registry = await readProjectFile('src/lib/permissions/registry.generated.ts');

	for (const permission of [
		'ACADEMIC_ASSESSMENT_READ_ASSIGNED',
		'ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT',
		'ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED',
		'ACADEMIC_ASSESSMENT_READ_SCHOOL',
		'ACADEMIC_ASSESSMENT_MANAGE_SCHOOL'
	]) {
		assert.match(registry, new RegExp(`${permission}:`));
	}
});

test('assessment api uses generated DTOs and offering-scoped endpoints', async () => {
	const api = await readProjectFile('src/lib/api/academicAssessments.ts');

	assert.match(api, /import type \{ components \} from '\$lib\/api\/generated\/school-api'/);
	for (const schema of [
		'AssessmentPlanSummary',
		'AssessmentPlanDetail',
		'SaveAssessmentPlanRequest',
		'AssessmentPhase',
		'AssessmentPhaseControl',
		'UpdateAssessmentPhaseControlRequest'
	]) {
		assert.match(api, new RegExp(`Schemas\\['${schema}'\\]`));
	}
	assert.match(api, /academicTermId:\s*string/);
	assert.match(api, /new URLSearchParams\(\{ academicTermId \}\)/);
	assert.match(
		api,
		/\/api\/academic\/assessments\/offerings\/\$\{encodeURIComponent\(offeringId\)\}/
	);
	assert.match(api, /\/api\/academic\/assessments\/plans/);
	assert.match(api, /\/api\/academic\/assessments\/phase-controls/);
	assert.doesNotMatch(api, /\/api\/academic\/assessments\/settings|submitAssessmentPlan/);
	for (const retiredToken of [
		['classroom', 'CourseId'].join(''),
		['academic', 'SemesterId'].join(''),
		'quick-scores'
	]) {
		assert.doesNotMatch(api, new RegExp(retiredToken));
	}
	assert.doesNotMatch(api, /courses\/\$\{/);
	assert.doesNotMatch(api, /interface AssessmentPlanSummary|interface AssessmentPlanDetail/);
});

test('generated contract publishes every assessment operation and DTO', async () => {
	const contract = await readProjectFile('src/lib/api/generated/school-api.ts');

	for (const operation of [
		'listAssessmentPlans',
		'getAssessmentPlan',
		'saveAssessmentPlan',
		'listAssessmentPhaseControls',
		'updateAssessmentPhaseControl'
	]) {
		assert.match(contract, new RegExp(`${operation}:`));
	}
	assert.match(contract, /academicTermId:\s*string/);
	assert.match(contract, /offeringId:\s*string/);
	assert.match(contract, /rowVersion\?:\s*number \| null/);
	assert.match(
		contract,
		/AssessmentPhaseCode: 'before_midterm' \| 'midterm' \| 'after_midterm' \| 'final'/
	);
	assert.doesNotMatch(contract, /submitAssessmentPlan:|getAssessmentSettings:/);
});

test('backend assessment model is term and offering scoped with optimistic locking', async () => {
	const model = await readRepoFile('backend-school/src/modules/academic/models/assessment.rs');

	assert.match(model, /pub academic_term_id: Uuid/);
	assert.match(model, /pub offering_id: Uuid/);
	assert.match(model, /pub learning_group_ids: Vec<Uuid>/);
	assert.match(model, /pub row_version: Option<i64>/);
	assert.match(model, /pub grading_policy: CourseGradingPolicy/);
	assert.match(model, /#\[into_params\(parameter_in = Query\)\]/);
	assert.doesNotMatch(model, /classroom_course_id|academic_semester_id/);
});

test('backend routes assessment plans through offering IDs and registers OpenAPI paths', async () => {
	const handler = await readRepoFile('backend-school/src/modules/academic/handlers/assessment.rs');
	const router = await readRepoFile('backend-school/src/modules/academic.rs');
	const contract = await readRepoFile('backend-school/src/api_contract.rs');

	assert.match(handler, /path = "\/api\/academic\/assessments\/offerings\/\{offering_id\}"/);
	assert.match(handler, /path = "\/api\/academic\/assessments\/phase-controls"/);
	assert.match(handler, /path = "\/api\/academic\/assessments\/phase-controls\/\{control_id\}"/);
	assert.match(router, /\/assessments\/offerings\/\{offering_id\}/);
	assert.match(router, /\/assessments\/phase-controls/);
	assert.match(contract, /crate::modules::academic::handlers::assessment::get_assessment_plan/);
	assert.match(contract, /AssessmentPlanDetail/);
	assert.doesNotMatch(router, /quick-scores|assessments\/courses|\/submit|\/settings/);
});

test('assessment route requires an explicit term context', async () => {
	const meta = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.ts');
	const page = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.svelte');

	assert.match(meta, /academicContext:\s*'term_required'/);
	assert.match(meta, /permission:\s*PERMISSION_MODULES\.ACADEMIC_ASSESSMENT/);
	assert.match(page, /getAcademicContextStore/);
	assert.match(page, /state\.selected\.academicTermId/);
	assert.match(page, /listAssessmentPlans\(\{ academicTermId: termId \}\)/);
	assert.match(page, /เลือกภาคเรียนก่อน/);
	assert.doesNotMatch(page, /getAcademicStructure|listClassrooms|selectedSemesterId/);
});

test('assessment workspace exposes offering snapshots instead of classroom course rows', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.svelte');

	assert.match(page, /plan\.offeringId/);
	assert.match(page, /plan\.offeringCode/);
	assert.match(page, /plan\.offeringName/);
	assert.match(page, /plan\.subjectVersionDisplayLabel/);
	assert.match(page, /plan\.learningGroupCount/);
	assert.match(page, /plan\.readiness\.totalScore/);
	assert.match(page, /plan\.readiness\.expectedTotalScore/);
	assert.match(page, /plan\.assessmentCoordinatorName/);
	assert.match(page, /plan\.phases/);
	for (const retiredToken of [
		['classroom', 'CourseId'].join(''),
		['classroom', 'Name'].join(''),
		['classroom', 'Id'].join('')
	]) {
		assert.doesNotMatch(page, new RegExp(retiredToken));
	}
});

test('assessment workspace guides an empty term to Learning Delivery without extra readiness requests', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.svelte');

	assert.match(page, /AcademicPrerequisiteNotice/);
	assert.match(page, /สร้างรายการเปิดสอนก่อนกำหนดโครงสร้างคะแนน/);
	assert.match(page, /href: '\/staff\/academic\/delivery'/);
	assert.doesNotMatch(page, /getLearningDeliveryManagementOptions|getCurriculumProgramWorkspace/);
});

test('assessment editor fixes four phases, assigns a coordinator, and auto-saves', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.svelte');

	for (const symbol of ['phaseCodes', 'draftCoordinatorId', 'persistDraft', 'saveAssessmentPlan']) {
		assert.match(page, new RegExp(symbol));
	}
	for (const phase of ['before_midterm', 'midterm', 'after_midterm', 'final']) {
		assert.match(page, new RegExp(`'${phase}'`));
	}
	for (const mode of ['none', 'in_timetable', 'outside_timetable']) {
		assert.match(page, new RegExp(`value: '${mode}'`));
	}
	assert.match(page, /bind:value=\{phase\.maxScore\}/);
	assert.match(page, /bind:value=\{phase\.examDurationMinutes\}/);
	assert.match(page, /setTimeout\(\(\) => void persistDraft\(\), 750\)/);
	assert.match(page, /บันทึกอัตโนมัติเมื่อแก้ไข/);
	for (const retiredSymbol of [
		'addCategory',
		'removeCategory',
		'addItem',
		'removeItem',
		'submitAssessmentPlan',
		"value: 'practical'"
	]) {
		assert.doesNotMatch(page, new RegExp(retiredSymbol));
	}
});

test('assessment save sends rowVersion and keeps dirty draft on conflicts', async () => {
	const api = await readProjectFile('src/lib/api/academicAssessments.ts');
	const page = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.svelte');
	const service = await readRepoFile(
		'backend-school/src/modules/academic/services/assessment_service.rs'
	);

	assert.match(page, /rowVersion:\s*detail\.rowVersion \?\? null/);
	assert.match(page, /registerAcademicContextDirtySource/);
	assert.match(page, /\(\) => dirty \|\| saving/);
	assert.match(page, /if \(dirty\) \{/);
	assert.match(api, /response\.status === 409/);
	assert.match(api, /เก็บข้อมูลที่แก้ไว้/);
	assert.match(service, /row_version/);
	assert.match(service, /conflict|Conflict/i);

	const saveHandler = page.slice(page.indexOf('async function persistDraft'));
	const catchBlock = saveHandler.slice(
		saveHandler.indexOf('} catch (error)'),
		saveHandler.indexOf('} finally')
	);
	assert.doesNotMatch(catchBlock, /draftPhases\s*=|dirty\s*=\s*false/);
});

test('assessment phase controls and coordinator authorization replace the global teacher switch', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/assessments/+page.svelte');
	const service = await readRepoFile(
		'backend-school/src/modules/academic/services/assessment_service.rs'
	);
	const capabilityBlock = page.slice(
		page.indexOf('const canRead'),
		page.indexOf('const filteredPlans')
	);

	assert.match(page, /phaseControls/);
	assert.match(page, /togglePhaseControl/);
	assert.match(page, /itemEditingEnabled/);
	assert.match(page, /scoreEntryEnabled/);
	assert.match(page, /canManageSchool/);
	assert.match(page, /ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED/);
	assert.match(page, /<Switch/);
	assert.match(service, /require_phase_controls_manage_access/);
	assert.match(service, /allowed_coordinator/);
	assert.match(service, /เฉพาะผู้รับผิดชอบโครงสร้างคะแนนหรือผู้ดูแลวิชาการเท่านั้น/);
	assert.doesNotMatch(page, /teacherAccessEnabled|toggleTeacherAccess/);
	assert.doesNotMatch(capabilityBlock, /LEARNING_OFFERING/);
});
