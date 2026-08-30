import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const readProjectFile = (relativePath) => readFile(path.join(projectRoot, relativePath), 'utf8');

test('timetable editor sends the exact selected instructors for create and update', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/timetable/+page.svelte');

	assert.match(page, /import TimetableInstructorPicker from/);
	assert.match(page, /let formInstructorIds = \$state<string\[\]>\(\[\]\)/);
	assert.doesNotMatch(page, /instructorIds:\s*\[\]/);
	assert.ok(
		(page.match(/instructorIds:\s*formInstructorIds/g) ?? []).length >= 2,
		'create and update must both send formInstructorIds'
	);
	assert.match(page, /entry\.instructors\.map\(\(item\) => item\.userId\)/);
	assert.match(page, /teacherAssignments\.length === 1/);
	assert.match(page, /<TimetableInstructorPicker/);
	assert.match(page, /ครูผู้สอนของคาบนี้/);
	assert.match(page, /unavailableSelectedInstructors/);
	assert.match(page, /ไม่ได้อยู่ในช่วงวันที่ของรุ่นตารางสอนนี้/);
	assert.match(page, /removeUnavailableInstructor/);
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
	const page = await readProjectFile('src/routes/(app)/staff/academic/timetable/+page.svelte');

	assert.match(page, /selectedEntry\.instructors/);
	assert.match(page, /instructor\.displayName/);
	assert.match(page, /canEditSelected/);
});
