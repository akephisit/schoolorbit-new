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
