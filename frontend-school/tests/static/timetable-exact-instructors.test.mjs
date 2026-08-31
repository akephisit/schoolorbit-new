import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const readProjectFile = (relativePath) => readFile(path.join(projectRoot, relativePath), 'utf8');

test('timetable editor sends the exact selected instructors for create and update', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/timetable/+page.svelte');
	const inspector = await readProjectFile(
		'src/lib/components/academic/timetable/TimetableEntryInspector.svelte'
	);

	assert.match(page, /import TimetableInstructorPicker from/);
	assert.match(page, /entry\.instructors\.map\(\(instructor\) => instructor\.userId\)/);
	assert.match(page, /instructorIds:\s*preview\.normalizedCandidate\.instructorIds/);
	assert.match(page, /pendingDemandInstructorIds/);
	assert.match(page, /<TimetableInstructorPicker/);
	assert.match(inspector, /entry\?\.instructors\.map\(\(teacher\) => teacher\.userId\)/);
	assert.match(inspector, /instructorIds:\s*selectedInstructorIds/);
	assert.doesNotMatch(page, /teacherAssignments|formInstructorIds/);
});

test('instructor picker is explicit accessible and keyed by teacher identity', async () => {
	const picker = await readProjectFile(
		'src/lib/components/academic/timetable/TimetableInstructorPicker.svelte'
	);

	assert.match(picker, /value = \$bindable<string\[\]>\(\[\]\)/);
	assert.match(picker, /type="button"/);
	assert.match(picker, /aria-pressed=/);
	assert.match(picker, /\{#each options as option \(option\.id\)\}/);
	assert.match(picker, /ครูผู้สอนของคาบนี้/);
	assert.match(picker, /ยังไม่มีครูผู้สอนที่เลือกได้/);
});

test('published timetable exposes exact instructor names without an editable picker', async () => {
	const card = await readProjectFile(
		'src/lib/components/academic/timetable/TimetableLessonCard.svelte'
	);
	const inspector = await readProjectFile(
		'src/lib/components/academic/timetable/TimetableEntryInspector.svelte'
	);

	assert.match(card, /entry\.instructors\.map\(\(teacher\) => teacher\.displayName\)/);
	assert.match(inspector, /disabled=\{readOnly \|\| busy\}/);
	assert.match(inspector, /\{#if entry && !readOnly\}/);
});
