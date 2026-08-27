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
