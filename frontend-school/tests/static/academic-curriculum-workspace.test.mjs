import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = join(dirname(fileURLToPath(import.meta.url)), '../..');

async function readProjectFile(relativePath) {
	return readFile(join(projectRoot, relativePath), 'utf8');
}

test('curriculum workspace clients use generated contracts', async () => {
	const api = await readProjectFile('src/lib/api/academic-core.ts');

	assert.match(api, /getCurriculumOverview/);
	assert.match(api, /getCurriculumCreateOptions/);
	assert.match(api, /getCurriculumManagementOptions/);
	assert.match(api, /operations\['getCurriculumOverview'\]/);
	assert.doesNotMatch(api, /ApiResponse<unknown>|Record<string, unknown>| as Curriculum/);
});

test('curriculum overview is read-first and uses labeled grade selection', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/curricula/+page.svelte');
	const table = await readProjectFile(
		'src/lib/components/academic-core/CurriculumOverviewTable.svelte'
	).catch(() => '');

	assert.match(page, /getCurriculumOverview/);
	assert.match(page, /canManageAcademicCurriculum/);
	assert.doesNotMatch(page, /getCurriculumCreateOptions\([\s\S]*onMount/);
	assert.doesNotMatch(page, /gradeLevelIds:\s*''/);
	assert.doesNotMatch(page, /รหัสระดับชั้น/);
	assert.match(table, /startAcademicYearName/);
	assert.match(table, /studyProgramCount/);
});

test('curriculum detail is deep-linked and uses labeled management options', async () => {
	const meta = await readProjectFile(
		'src/routes/(app)/staff/academic/curricula/[id]/+page.ts'
	).catch(() => '');
	const page = await readProjectFile(
		'src/routes/(app)/staff/academic/curricula/[id]/+page.svelte'
	).catch(() => '');
	const editor = await readProjectFile(
		'src/lib/components/academic-core/CurriculumProgramEditor.svelte'
	);

	assert.match(meta, /_meta\s*=\s*\{[\s\S]*access:/);
	assert.doesNotMatch(meta, /menu:/);
	assert.match(page, /getCurriculumProgramWorkspace/);
	assert.match(page, /getCurriculumManagementOptions/);
	assert.match(editor, /catalogVersions/);
	assert.match(editor, /gradeLevels/);
	assert.doesNotMatch(editor, /catalogVersionId[^\n]*<Input/);
	assert.doesNotMatch(editor, /gradeLevelId[^\n]*<Input/);
});
