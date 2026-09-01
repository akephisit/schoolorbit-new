import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const readProjectFile = (relativePath) => readFile(path.join(projectRoot, relativePath), 'utf8');

test('teacher changes use generated typed item and handoff contracts', async () => {
	const api = await readProjectFile('src/lib/api/learning-delivery.ts');

	for (const operation of ['previewTeacherHandoff', 'applyTeacherHandoff']) {
		assert.match(api, new RegExp(`operations\\['${operation}'\\]`));
	}
	for (const schema of [
		'PreviewTeacherHandoffRequest',
		'ApplyTeacherHandoffRequest',
		'TeacherHandoffPreview',
		'ApplyTeacherHandoffResponse'
	]) {
		assert.match(api, new RegExp(`Schemas\\['${schema}'\\]`));
	}
	assert.doesNotMatch(api, /ApiResponse<unknown>|Record<string, unknown>|academic_term_id/);
});

test('teacher change form uses the bounded management option response', async () => {
	const form = await readProjectFile(
		'src/lib/components/learning-delivery/AcademicTeacherChangeForm.svelte'
	);

	assert.match(form, /managementOptions\.learningGroups/);
	assert.match(form, /managementOptions\.teachers/);
	assert.match(form, /add_group_teacher/);
	assert.match(form, /adjust_group_teacher_role/);
	assert.match(form, /stop_group_teacher/);
	assert.match(form, /หลังเผยแพร่เท่านั้น/);
	assert.doesNotMatch(form, /listLearningGroups|lookupStaff|<select(?:\s|>)/i);
});

test('handoff stays explicit, previews conflicts, and never applies manual mode', async () => {
	const panel = await readProjectFile(
		'src/lib/components/learning-delivery/TeacherHandoffPanel.svelte'
	);

	assert.match(panel, /assign_one/);
	assert.match(panel, /assign_coteachers/);
	assert.match(panel, /manual/);
	assert.match(panel, /previewTeacherHandoff/);
	assert.match(panel, /applyTeacherHandoff/);
	assert.match(panel, /preview\.conflicts\.length/);
	assert.match(panel, /preview\?\.canApply/);
	assert.match(panel, /preview\.timetableRoute/);
	assert.match(panel, /mode === 'manual'/);
	assert.doesNotMatch(panel, /<select(?:\s|>)/i);

	const manualBranch = panel.slice(panel.indexOf("mode === 'manual'"));
	assert.doesNotMatch(manualBranch.slice(0, 800), /applyTeacherHandoff\(/);
});

test('change set and readiness panels expose teacher handoff without reviving direct teacher edits', async () => {
	const changePanel = await readProjectFile(
		'src/lib/components/learning-delivery/AcademicChangeSetPanel.svelte'
	);
	const readiness = await readProjectFile(
		'src/lib/components/learning-delivery/AcademicChangeReadiness.svelte'
	);

	assert.match(changePanel, /AcademicTeacherChangeForm/);
	assert.match(changePanel, /TeacherHandoffPanel/);
	assert.match(changePanel, /จัดการคาบที่ได้รับผลกระทบ/);
	assert.doesNotMatch(changePanel, /replaceLearningGroupTeachers/);
	assert.match(readiness, /teacherFindings/);
	assert.match(readiness, /stopped_teacher_still_scheduled/);
	assert.match(readiness, /entry_instructor_not_effective/);
});

test('teacher handoff findings remain in readiness and link to the canonical timetable', async () => {
	const readiness = await readProjectFile(
		'src/lib/components/learning-delivery/AcademicChangeReadiness.svelte'
	);
	const page = await readProjectFile('src/routes/(app)/staff/academic/timetable/+page.svelte');

	assert.match(readiness, /stopped_teacher_still_scheduled/);
	assert.match(readiness, /entry_instructor_not_effective/);
	assert.match(page, /blockTeacherIds/);
	assert.match(page, /updateTimetableBlock/);
});
