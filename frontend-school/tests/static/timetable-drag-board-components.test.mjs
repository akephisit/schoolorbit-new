import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const componentRoot = 'src/lib/components/academic/timetable';
const componentNames = [
	'TimetableWorkspaceHeader.svelte',
	'TimetableViewSelector.svelte',
	'TimetableUnscheduledTray.svelte',
	'TimetableBoard.svelte',
	'TimetableCell.svelte',
	'TimetableLessonCard.svelte',
	'TimetableEntryInspector.svelte',
	'TimetableMoveDialog.svelte'
];

const read = (relativePath) => readFile(path.join(projectRoot, relativePath), 'utf8');

test('timetable drag board is decomposed into presentation-only components', async () => {
	const sources = await Promise.all(componentNames.map((name) => read(`${componentRoot}/${name}`)));
	const combined = sources.join('\n');

	assert.doesNotMatch(
		combined,
		/\b(?:getTimetableWorkspace|previewTimetablePlacement|createTimetableEntry|updateTimetableEntry|swapTimetableEntries|deleteTimetableEntry)\s*\(/
	);
	assert.doesNotMatch(combined, /<select(?:\s|>)/i);
	assert.match(combined, /Select\.Root/);
	assert.match(combined, /\$props\(\)/);
});

test('lesson cards and cells expose drag state plus complete keyboard parity', async () => {
	const card = await read(`${componentRoot}/TimetableLessonCard.svelte`);
	const tray = await read(`${componentRoot}/TimetableUnscheduledTray.svelte`);
	const cell = await read(`${componentRoot}/TimetableCell.svelte`);
	const board = await read(`${componentRoot}/TimetableBoard.svelte`);

	assert.match(card, /draggable=/);
	assert.match(card, /aria-label=/);
	assert.match(card, /ย้ายคาบ/);
	assert.match(card, /แก้รายละเอียด/);
	assert.match(card, /นำออกจากตาราง/);
	assert.match(tray, /draggable=/);
	assert.match(tray, /onDragStartDemand/);
	assert.match(cell, /dayLabel/);
	assert.match(cell, /periodLabel/);
	assert.match(cell, /stateLabel/);
	assert.match(cell, /onActivateIntent/);
	assert.match(cell, /วางคาบที่นี่/);
	assert.match(board, /entry\.id.*row\.id|row\.id.*entry\.id/s);
	assert.match(board, /Escape/);
});

test('non-drag move dialog uses shadcn selectors for one day and one period', async () => {
	const dialog = await read(`${componentRoot}/TimetableMoveDialog.svelte`);

	assert.match(dialog, /Select\.Root/);
	assert.match(dialog, /selectedDay/);
	assert.match(dialog, /selectedPeriodId/);
	assert.match(dialog, /ย้ายคาบ/);
	assert.doesNotMatch(dialog, /ช่วงคาบ|สองคาบ|2\s*คาบ/);
});
