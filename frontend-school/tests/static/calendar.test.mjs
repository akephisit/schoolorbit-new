import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '../..');

async function readProjectFile(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

function stripComments(source) {
	return source
		.replace(/<!--[\s\S]*?-->/g, '')
		.replace(/\/\*[\s\S]*?\*\//g, '')
		.replace(/\/\/.*$/gm, '');
}

test('calendar permission registry and routes are wired', async () => {
	const registry = await readProjectFile('src/lib/permissions/registry.ts');
	const staffRoute = await readProjectFile('src/routes/(app)/staff/calendar/+page.ts');
	const studentRoute = await readProjectFile('src/routes/(app)/student/calendar/+page.ts');
	const parentRoute = await readProjectFile(
		'src/routes/(app)/parent/student/[id]/calendar/+page.ts'
	);

	assert.match(registry, /CALENDAR:\s*['"]calendar['"]/);
	assert.match(registry, /CALENDAR_READ_SCHOOL:\s*['"]calendar\.read\.school['"]/);
	assert.match(registry, /CALENDAR_MANAGE_SCHOOL:\s*['"]calendar\.manage\.school['"]/);
	assert.match(staffRoute, /permission:\s*PERMISSION_MODULES\.CALENDAR/);
	assert.match(studentRoute, /user_type:\s*['"]student['"]/);
	assert.match(parentRoute, /user_type:\s*['"]parent['"]/);
});

test('calendar frontend uses typed API client and shadcn primitives', async () => {
	const api = await readProjectFile('src/lib/api/calendar.ts');
	const staffPage = await readProjectFile('src/routes/(app)/staff/calendar/+page.svelte');
	const eventDialog = await readProjectFile('src/lib/components/calendar/CalendarEventDialog.svelte');
	const categoryDialog = await readProjectFile(
		'src/lib/components/calendar/CalendarCategoryDialog.svelte'
	);

	for (const name of [
		'CalendarEvent',
		'CalendarCategory',
		'CalendarEventTarget',
		'CreateCalendarEventRequest',
		'listCalendarEvents',
		'listMyCalendarEvents',
		'listChildCalendarEvents',
		'listPublicCalendarEvents'
	]) {
		assert.match(api, new RegExp(`\\b${name}\\b`));
	}

	assert.match(staffPage, /PageShell/);
	assert.match(staffPage, /PERMISSIONS\.CALENDAR_MANAGE_SCHOOL/);
	assert.doesNotMatch(stripComments(staffPage), /calendar\.(manage|read)\.school/);
	assert.match(eventDialog, /from '\$lib\/components\/ui\/dialog'/);
	assert.match(eventDialog, /from '\$lib\/components\/ui\/select'/);
	assert.match(eventDialog, /from '\$lib\/components\/ui\/checkbox'/);
	assert.match(categoryDialog, /from '\$lib\/components\/ui\/dialog'/);
	assert.match(categoryDialog, /from '\$lib\/components\/ui\/button'/);
});
