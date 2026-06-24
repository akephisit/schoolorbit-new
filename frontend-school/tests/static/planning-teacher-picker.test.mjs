import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '../..');
const planningPage = 'src/routes/(app)/staff/academic/planning/+page.svelte';

async function readProjectFile(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

function between(source, start, end) {
	const startIndex = source.indexOf(start);
	assert.notEqual(startIndex, -1, `missing section start: ${start}`);

	const endIndex = source.indexOf(end, startIndex + start.length);
	assert.notEqual(endIndex, -1, `missing section end: ${end}`);

	return source.slice(startIndex, endIndex);
}

test('planning teacher selectors use searchable comboboxes', async () => {
	const source = await readProjectFile(planningPage);
	const editDialog = between(source, '<!-- Edit Dialog -->', '<!-- Delete Dialog -->');
	const teamDialog = between(source, '<!-- Team Teaching Dialog -->', '</PageShell>');

	assert.match(source, /import \* as Popover from '\$lib\/components\/ui\/popover'/);
	assert.match(source, /import \* as Command from '\$lib\/components\/ui\/command'/);
	assert.match(source, /Check/);
	assert.match(source, /ChevronsUpDown/);

	assert.match(editDialog, /<Popover\.Root bind:open=\{editTeacherPickerOpen\}>/);
	assert.match(editDialog, /role="combobox"/);
	assert.match(editDialog, /<Command\.Input placeholder="ค้นหาครู\.\.\." \/>/);
	assert.match(editDialog, /<Command\.Item[\s\S]*onSelect=\{\(\) =>/);

	assert.match(teamDialog, /<Popover\.Root bind:open=\{teamTeacherPickerOpen\}>/);
	assert.match(teamDialog, /role="combobox"/);
	assert.match(teamDialog, /<Command\.Input placeholder="ค้นหาครู\.\.\." \/>/);
	assert.match(teamDialog, /<Command\.Item[\s\S]*onSelect=\{\(\) =>/);
});

test('planning team teacher add button shows request loading state', async () => {
	const source = await readProjectFile(planningPage);
	const teamDialog = between(source, '<!-- Team Teaching Dialog -->', '</PageShell>');

	assert.match(source, /import \{ LoadingButton,/);
	assert.match(source, /let teamInstructorAdding = \$state\(false\);/);
	assert.match(source, /teamInstructorAdding = true;/);
	assert.match(source, /teamInstructorAdding = false;/);

	assert.match(teamDialog, /<LoadingButton[\s\S]*loading=\{teamInstructorAdding\}/);
	assert.match(teamDialog, /disabled=\{!teamDialogSelectedInstructor \|\| teamInstructorAdding\}/);
	assert.match(teamDialog, /loadingLabel="กำลังเพิ่ม\.\.\."/);
});
