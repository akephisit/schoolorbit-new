import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(__dirname, '../..');

const readSource = (relativePath) => readFile(path.join(frontendRoot, relativePath), 'utf8');

test('catalog version controls use typed shadcn inputs instead of UUID text fields', async () => {
	const history = await readSource('src/lib/components/academic-core/CatalogVersionHistory.svelte');

	assert.match(history, /GradeLevelMultiSelect/);
	assert.match(history, /\* as Select from ['"]\$lib\/components\/ui\/select/);
	assert.match(history, /DatePicker/);
	assert.doesNotMatch(history, /รหัสระดับชั้น \(คั่นด้วยจุลภาค\)/);
	assert.doesNotMatch(history, /type=["']date["']/);
});

test('grade-level multi-select is built from local shadcn primitives', async () => {
	const multiselect = await readSource(
		'src/lib/components/academic-core/GradeLevelMultiSelect.svelte'
	);

	assert.match(multiselect, /\* as Popover/);
	assert.match(multiselect, /\* as Command/);
	assert.match(multiselect, /Checkbox/);
	assert.match(multiselect, /aria-label/);
	assert.doesNotMatch(multiselect, /option\.id\}\s*<\/span>/);
});

test('catalog presentation keeps one canonical set of human-readable choices', async () => {
	const presentation = await readSource('src/lib/academic-core/catalog-presentation.ts');

	assert.match(presentation, /SUBJECT_TYPE_OPTIONS/);
	assert.match(presentation, /ACTIVITY_TYPE_OPTIONS/);
	assert.match(presentation, /SCHEDULING_MODE_OPTIONS/);
	assert.match(presentation, /displayStateLabel/);
	assert.match(presentation, /formatEffectiveRange/);
});

test('subject catalog uses the responsive overview information architecture', async () => {
	const subjects = await readSource(
		'src/routes/(app)/staff/academic/catalog/subjects/+page.svelte'
	);

	assert.match(subjects, /getCatalogSubjectOverview/);
	assert.match(subjects, /\* as Table/);
	assert.match(subjects, /\* as Sheet/);
	assert.match(subjects, /\* as Select/);
	assert.match(subjects, /subjectHistoryCache/);
	assert.match(subjects, /historyRevision/);
	assert.match(subjects, /ownerOptions/);
	assert.match(subjects, /canManage=\{selected\.canManage\}/);
	assert.match(subjects, /formatEffectiveRange/);
	assert.match(subjects, /subject\.archivedAt/);
	assert.match(subjects, /md:hidden/);
	assert.match(subjects, /hidden md:block/);
	for (const label of ['ชื่อรายวิชา', 'ประเภท', 'ระดับชั้น', 'หน่วยกิต', 'สถานะ']) {
		assert.match(subjects, new RegExp(label));
	}
});

test('activity catalog uses the responsive overview information architecture', async () => {
	const activities = await readSource(
		'src/routes/(app)/staff/academic/catalog/activities/+page.svelte'
	);

	assert.match(activities, /getCatalogActivityOverview/);
	assert.match(activities, /\* as Table/);
	assert.match(activities, /\* as Sheet/);
	assert.match(activities, /\* as Select/);
	assert.match(activities, /activityHistoryCache/);
	assert.match(activities, /historyRevision/);
	assert.match(activities, /ownerOptions/);
	assert.match(activities, /canManage=\{selected\.canManage\}/);
	assert.match(activities, /formatEffectiveRange/);
	assert.match(activities, /activity\.archivedAt/);
	assert.match(activities, /ACTIVITY_TYPE_OPTIONS/);
	assert.match(activities, /SCHEDULING_MODE_OPTIONS/);
	for (const label of [
		'ชื่อกิจกรรม',
		'ประเภทกิจกรรม',
		'รูปแบบการจัด',
		'ระดับชั้น',
		'ชั่วโมง',
		'สถานะ'
	]) {
		assert.match(activities, new RegExp(label));
	}
});

test('curriculum requirement form reports and focuses a missing program selection', async () => {
	const editor = await readSource(
		'src/lib/components/academic-core/CurriculumProgramEditor.svelte'
	);

	assert.match(editor, /requirementProgramError/);
	assert.match(editor, /requirementProgramTrigger\?\.focus/);
	assert.match(editor, /role="alert"/);
	assert.match(editor, /aria-invalid/);
});
