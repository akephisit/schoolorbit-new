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
