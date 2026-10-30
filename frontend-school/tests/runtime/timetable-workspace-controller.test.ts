import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { compileModule } from 'svelte/compiler';
import ts from 'typescript';
import { collaborationWorkspace } from './timetable-collaboration-fixture.ts';
import { timetableIds } from '../e2e/timetable-test-harness.ts';

// Execute the owning runes module with the installed client compiler, including derived rows.
const filename = new URL(
	'../../src/lib/academic/timetable/workspace-controller.svelte.ts',
	import.meta.url
);
const source = ts.transpileModule(await readFile(filename, 'utf8'), {
	compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext }
}).outputText;
const code = compileModule(source, { generate: 'client', filename: filename.pathname })
	.js.code.replace(
		"'svelte/internal/client'",
		JSON.stringify(import.meta.resolve('svelte/internal/client'))
	)
	.replace("'./board-state'", JSON.stringify(new URL('./board-state.ts', filename).href));
const {
	createTimetableWorkspaceController
}: typeof import('../../src/lib/academic/timetable/workspace-controller.svelte.ts') = await import(
	`data:text/javascript;base64,${Buffer.from(code).toString('base64')}`
);

const secondHomeroom = '81000000-0000-4000-8000-000000000202';
function workspace() {
	const value = collaborationWorkspace();
	value.homerooms.push({
		...value.homerooms[0],
		id: secondHomeroom,
		roomNumber: '2',
		name: 'ม.1/2'
	});
	value.staff = [timetableIds.teacherA, timetableIds.teacherB].map((id, index) => ({
		id,
		displayName: `ครู ${index}`,
		status: 'active',
		subjectGroups: []
	}));
	return value;
}

test('remembers independent valid owners through every view roundtrip and same-view selection', () => {
	const controller = createTimetableWorkspaceController(workspace());
	controller.selectOwner(secondHomeroom);
	controller.setView('teacher');
	controller.selectOwner(timetableIds.teacherB);
	controller.setView('learning_group');
	controller.selectOwner(timetableIds.groupB);
	for (const [view, owner] of [
		['homeroom', secondHomeroom],
		['teacher', timetableIds.teacherB],
		['learning_group', timetableIds.groupB]
	] as const) {
		controller.setView(view);
		assert.equal(controller.selectedOwnerId, owner);
		controller.setView(view);
		assert.equal(controller.selectedOwnerId, owner);
		controller.selectOwner(timetableIds.room);
		assert.equal(controller.selectedOwnerId, owner);
	}
});

test('canonical scope loss repairs selected and inactive-view memories without restoring removed owners on regrant', () => {
	const controller = createTimetableWorkspaceController(workspace());
	controller.selectOwner(secondHomeroom);
	controller.setView('teacher');
	controller.selectOwner(timetableIds.teacherB);
	controller.setView('learning_group');
	controller.selectOwner(timetableIds.groupB);
	const next = workspace();
	next.homerooms = next.homerooms.filter((row) => row.id !== secondHomeroom);
	next.staff = next.staff.filter((row) => row.id !== timetableIds.teacherB);
	next.learningGroups = next.learningGroups.filter((row) => row.id !== timetableIds.groupB);
	controller.setWorkspace(next);
	assert.equal(controller.selectedOwnerId, timetableIds.groupA);
	controller.setView('teacher');
	assert.equal(controller.selectedOwnerId, timetableIds.teacherA);
	controller.setView('homeroom');
	assert.equal(controller.selectedOwnerId, timetableIds.homeroom);
	controller.setWorkspace(workspace());
	controller.setView('teacher');
	assert.equal(controller.selectedOwnerId, timetableIds.teacherA);
});

test('same-workspace snapshot transfers only selection values to a new interaction identity and validates new rows', () => {
	const original = createTimetableWorkspaceController(workspace());
	original.setEditing(true);
	original.setView('teacher');
	original.selectOwner(timetableIds.teacherB);
	const snapshot = original.ownerSelections();
	const next = createTimetableWorkspaceController(workspace(), snapshot);
	assert.notEqual(next, original);
	assert.equal(next.editing, false);
	assert.equal(next.dragSource, null);
	next.setView('teacher');
	assert.equal(next.selectedOwnerId, timetableIds.teacherB);
	snapshot.teacher = timetableIds.teacherA;
	next.setView('teacher');
	assert.equal(next.selectedOwnerId, timetableIds.teacherB);
	const restricted = workspace();
	restricted.staff = [];
	const denied = createTimetableWorkspaceController(restricted, original.ownerSelections());
	denied.setView('teacher');
	assert.equal(denied.selectedOwnerId, null);
});

test('a new workspace version resets remembered choices and never uses an owner from another view', () => {
	const controller = createTimetableWorkspaceController(workspace());
	controller.setView('teacher');
	controller.selectOwner(timetableIds.teacherB);
	const next = workspace();
	next.version = { ...next.version, id: timetableIds.publishedVersion };
	controller.setWorkspace(next);
	assert.equal(controller.selectedOwnerId, timetableIds.teacherA);
	controller.setView('homeroom');
	assert.equal(controller.selectedOwnerId, timetableIds.homeroom);
});
