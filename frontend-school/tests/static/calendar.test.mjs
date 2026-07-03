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

function interfaceBody(source, name) {
	const match = source.match(new RegExp(`export interface ${name}\\s*{([\\s\\S]*?)\\n}`));
	assert.ok(match, `Expected exported interface ${name}`);
	return match[1];
}

function functionBody(source, name) {
	const match = source.match(new RegExp(`function ${name}\\([^)]*\\)\\s*{([\\s\\S]*?)\\n}`));
	assert.ok(match, `Expected function ${name}`);
	return match[1];
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

	for (const name of [
		'CalendarEvent',
		'CalendarPublicEvent',
		'CalendarCategory',
		'CalendarEventTarget',
		'CalendarEventTargetInput',
		'CreateCalendarEventRequest',
		'listCalendarEvents',
		'listMyCalendarEvents',
		'listChildCalendarEvents',
		'listPublicCalendarEvents'
	]) {
		assert.match(api, new RegExp(`\\b${name}\\b`));
	}

	const target = interfaceBody(api, 'CalendarEventTarget');
	const targetInput = interfaceBody(api, 'CalendarEventTargetInput');
	const publicFilters = interfaceBody(api, 'CalendarPublicEventFilters');
	const publicQuery = functionBody(api, 'publicCalendarQuery');

	assert.match(target, /\bid:\s*string;/);
	assert.match(target, /\baudienceType:\s*CalendarAudienceType;/);
	assert.match(target, /\bclassRoomId\?:\s*string \| null;/);
	assert.doesNotMatch(target, /\baudience:\s*CalendarAudienceType;/);
	assert.doesNotMatch(target, /\bclassroomId\?:\s*string \| null;/);
	assert.match(targetInput, /\baudienceType:\s*CalendarAudienceType;/);
	assert.match(targetInput, /\bclassRoomId\?:\s*string \| null;/);
	assert.doesNotMatch(targetInput, /\baudience:\s*CalendarAudienceType;/);
	assert.doesNotMatch(targetInput, /\bclassroomId\?:\s*string \| null;/);
	assert.doesNotMatch(targetInput, /\bid[?:]?:\s*string;/);
	assert.match(api, /targets:\s*CalendarEventTargetInput\[];/);
	assert.match(api, /export interface CalendarPublicEvent\s*{/);
	assert.doesNotMatch(api, /CalendarPublicEvent\s*=\s*Omit/);
	assert.match(publicFilters, /categoryId\?:\s*string;/);
	assert.doesNotMatch(publicFilters, /audience\?:/);
	assert.doesNotMatch(publicFilters, /visibility\?:/);
	assert.doesNotMatch(publicQuery, /\baudience\b/);
	assert.doesNotMatch(publicQuery, /\bvisibility\b/);
	assert.match(api, /listPublicCalendarEvents[\s\S]*Promise<CalendarPublicEvent\[]>/);
	assert.match(api, /listPublicCalendarEvents[\s\S]*publicCalendarQuery\(filters\)/);

	const staffPage = await readProjectFile('src/routes/(app)/staff/calendar/+page.svelte');
	const eventDialog = await readProjectFile(
		'src/lib/components/calendar/CalendarEventDialog.svelte'
	);
	const categoryDialog = await readProjectFile(
		'src/lib/components/calendar/CalendarCategoryDialog.svelte'
	);

	assert.match(staffPage, /PageShell/);
	assert.match(staffPage, /PERMISSIONS\.CALENDAR_MANAGE_SCHOOL/);
	assert.doesNotMatch(stripComments(staffPage), /calendar\.(manage|read)\.school/);
	assert.match(eventDialog, /from '\$lib\/components\/ui\/dialog'/);
	assert.match(eventDialog, /from '\$lib\/components\/ui\/select'/);
	assert.match(eventDialog, /from '\$lib\/components\/ui\/checkbox'/);
	assert.match(categoryDialog, /from '\$lib\/components\/ui\/dialog'/);
	assert.match(categoryDialog, /from '\$lib\/components\/ui\/button'/);
});
