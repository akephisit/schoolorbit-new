import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const read = (relativePath) => readFile(path.join(projectRoot, relativePath), 'utf8');

test('canonical timetable board keeps API mutations in the route controller', async () => {
	const componentNames = [
		'TimetableWorkspaceHeader.svelte',
		'TimetableViewSelector.svelte',
		'TimetableUnscheduledTray.svelte',
		'TimetableBoard.svelte',
		'TimetableCell.svelte',
		'TimetableLessonCard.svelte',
		'TimetableInstructorPicker.svelte'
	];
	const sources = await Promise.all(
		componentNames.map((name) => read(`src/lib/components/academic/timetable/${name}`))
	);
	const combined = sources.join('\n');

	assert.doesNotMatch(combined, /apiClient\.|getTimetableBlockWorkspace\s*\(/);
	assert.doesNotMatch(combined, /<select(?:\s|>)/i);
	assert.match(combined, /Select\.Root|Popover\.Root/);
	assert.match(combined, /\$props\(\)/);
});

test('lesson tray and board expose native drag with tap-to-place parity', async () => {
	const page = await read('src/routes/(app)/staff/academic/timetable/+page.svelte');
	const card = await read('src/lib/components/academic/timetable/TimetableLessonCard.svelte');
	const tray = await read('src/lib/components/academic/timetable/TimetableUnscheduledTray.svelte');
	const cell = await read('src/lib/components/academic/timetable/TimetableCell.svelte');
	const board = await read('src/lib/components/academic/timetable/TimetableBoard.svelte');

	assert.match(card, /draggable=/);
	assert.match(card, /data-block-id=/);
	assert.match(card, /aria-label=/);
	assert.match(card, /onRemove/);
	assert.doesNotMatch(card, /ย้ายคาบ|แก้รายละเอียด/);
	assert.match(tray, /draggable=/);
	assert.match(tray, /eligibleInstructors/);
	assert.match(tray, /onDragStartDemand/);
	assert.match(cell, /onActivateIntent/);
	assert.match(cell, /วางคาบที่นี่/);
	assert.match(board, /block\.id.*row\.id|row\.id.*block\.id/s);
	assert.match(board, /Escape/);
	assert.match(page, /previewTimetableBlockPlacement/);
	assert.match(page, /swapTimetableBlocks/);
	assert.match(page, /controller\.preview\.conflicts/);
	assert.match(page, /conflict\.message/);
	assert.match(page, /previewCellKey === targetCellKey[\s\S]*controller\.preview/);
});

test('teacher projection and structural blocks are first-class scheduling views', async () => {
	const page = await read('src/routes/(app)/staff/academic/timetable/+page.svelte');
	const selector = await read('src/lib/components/academic/timetable/TimetableViewSelector.svelte');
	const boardState = await read('src/lib/academic/timetable/board-state.ts');

	assert.match(selector, /onViewChange\('teacher'\)/);
	assert.match(boardState, /blockTeacherIds/);
	assert.match(boardState, /blockBelongsToRow/);
	assert.match(page, /createStructuralTimetableBlocks/);
	assert.match(page, /allHomerooms/);
	assert.match(page, /allTeachers/);
	assert.match(page, /removeTimetableBlockTarget/);
	assert.match(page, /deleteTimetableBlockSeries/);
});
