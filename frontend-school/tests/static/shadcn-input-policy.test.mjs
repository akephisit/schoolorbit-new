import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const frontendRoot = path.resolve(import.meta.dirname, '../..');

async function svelteFiles(directory) {
	const entries = await readdir(path.join(frontendRoot, directory), { withFileTypes: true });
	const files = [];
	for (const entry of entries) {
		const relativePath = path.join(directory, entry.name);
		if (entry.isDirectory()) files.push(...(await svelteFiles(relativePath)));
		else if (entry.name.endsWith('.svelte')) files.push(relativePath);
	}
	return files;
}

test('application forms use shared shadcn select and date controls', async () => {
	for (const file of await svelteFiles('src')) {
		const source = await readFile(path.join(frontendRoot, file), 'utf8');
		assert.doesNotMatch(source, /<select(?:\s|>)/, `${file} must use shadcn Select`);
		assert.doesNotMatch(source, /type=["']date["']/, `${file} must use DatePicker`);
	}
});

test('calendar captions use shadcn Select while preserving the placeholder contract', async () => {
	const caption = await readFile(
		path.join(frontendRoot, 'src/lib/components/ui/calendar/calendar-caption.svelte'),
		'utf8'
	);
	const calendar = await readFile(
		path.join(frontendRoot, 'src/lib/components/ui/calendar/calendar.svelte'),
		'utf8'
	);

	assert.match(caption, /\* as Select/);
	assert.match(caption, /placeholder\s*=\s*\$bindable/);
	assert.match(calendar, /bind:placeholder/);
	assert.match(caption, /month\.set\(\{ month:/);
	assert.match(caption, /month\.set\(\{ year:/);
	assert.doesNotMatch(caption, /placeholder\.set\(/);
	assert.doesNotMatch(caption, /CalendarMonthSelect|CalendarYearSelect/);
});

test('DatePicker exposes accessible migration props and safe clear behavior', async () => {
	const datePicker = await readFile(
		path.join(frontendRoot, 'src/lib/components/ui/date-picker/DatePicker.svelte'),
		'utf8'
	);

	for (const prop of ['disabled', 'required', 'clearable', 'ariaLabel']) {
		assert.match(datePicker, new RegExp(`\\b${prop}\\b`));
	}
	assert.match(datePicker, /clearable\s*&&\s*value\s*&&\s*!disabled/);
});

test('academic editors keep controlled shadcn Select values', async () => {
	const periods = await readFile(
		path.join(frontendRoot, 'src/lib/components/academic-core/AcademicYearTermEditor.svelte'),
		'utf8'
	);
	const timetable = await readFile(
		path.join(frontendRoot, 'src/routes/(app)/staff/academic/timetable/+page.svelte'),
		'utf8'
	);

	assert.match(periods, /value=\{selectedBellScheduleId\}/);
	assert.match(periods, /selectedBellScheduleId = id/);
	assert.match(timetable, /bind:value=\{scheduleSelectValue\}/);
	assert.match(timetable, /bind:value=\{targetSelectValue\}/);
	assert.match(timetable, /scheduleSelectValue = selectedScheduleId/);
	assert.match(timetable, /targetSelectValue = selectedTargetId/);
});
