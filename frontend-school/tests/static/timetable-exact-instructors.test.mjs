import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const read = (relativePath) => readFile(path.join(projectRoot, relativePath), 'utf8');

test('ordinary blocks send the exact teacher subset selected before placement', async () => {
	const page = await read('src/routes/(app)/staff/academic/timetable/+page.svelte');
	const tray = await read('src/lib/components/academic/timetable/TimetableUnscheduledTray.svelte');

	assert.match(tray, /instructorChoices/);
	assert.match(tray, /selectedInstructorIds/);
	assert.match(tray, /toggleInstructor/);
	assert.match(tray, /aria-pressed=\{selectedIds\.includes\(teacher\.teacherId\)\}/);
	assert.match(tray, /candidate:[\s\S]*instructorIds:\s*selectedInstructorIds\(demand\)/);
	assert.match(page, /instructorIds:\s*dragSource\.candidate\.instructorIds/);
	assert.match(page, /instructorIds:[\s\S]*editInstructorIds/);
	assert.doesNotMatch(page, /teacherAssignments|formInstructorIds/);
});

test('instructor picker remains explicit accessible and keyed by teacher identity', async () => {
	const picker = await read(
		'src/lib/components/academic/timetable/TimetableInstructorPicker.svelte'
	);

	assert.match(picker, /value = \$bindable<string\[\]>\(\[\]\)/);
	assert.match(picker, /type="button"/);
	assert.match(picker, /aria-pressed=/);
	assert.match(picker, /\{#each options as option \(option\.id\)\}/);
	assert.match(picker, /ครูผู้สอนของคาบนี้/);
});

test('published cards expose canonical instructor names without edit controls', async () => {
	const card = await read('src/lib/components/academic/timetable/TimetableLessonCard.svelte');
	const page = await read('src/routes/(app)/staff/academic/timetable/+page.svelte');

	assert.match(card, /group\.instructors\.map\(\(teacher\) => teacher\.displayName\)/);
	assert.match(card, /block\.teachers\.map\(\(teacher\) => teacher\.displayName\)/);
	assert.match(card, /\{#if canEdit\}/);
	assert.match(page, /controller\?\.canEdit/);
});
