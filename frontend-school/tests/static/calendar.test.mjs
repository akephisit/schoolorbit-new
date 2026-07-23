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

function generatedSchemaBody(source, name) {
	const marker = new RegExp(`^[\\t ]*${name}:\\s*\\{`, 'm').exec(source);
	assert.ok(marker, `Expected generated schema ${name}`);
	const opening = source.indexOf('{', marker.index);
	let depth = 0;
	for (let index = opening; index < source.length; index += 1) {
		if (source[index] === '{') depth += 1;
		if (source[index] === '}') depth -= 1;
		if (depth === 0) return source.slice(opening + 1, index);
	}
	assert.fail(`Expected balanced generated schema ${name}`);
}

function functionBody(source, name) {
	const match = source.match(new RegExp(`function ${name}\\([^)]*\\)\\s*{([\\s\\S]*?)\\n}`));
	assert.ok(match, `Expected function ${name}`);
	return match[1];
}

function svelteFunctionBody(source, name) {
	const startPattern = new RegExp(`function ${name}\\([^)]*\\)\\s*{`);
	const match = startPattern.exec(source);
	assert.ok(match, `Expected function ${name}`);
	let depth = 1;
	let index = match.index + match[0].length;

	while (index < source.length && depth > 0) {
		const char = source[index];
		if (char === '{') depth += 1;
		if (char === '}') depth -= 1;
		index += 1;
	}

	assert.equal(depth, 0, `Expected balanced function body for ${name}`);
	return source.slice(match.index + match[0].length, index - 1);
}

test('calendar API client uses current typed contracts', async () => {
	const api = await readProjectFile('src/lib/api/calendar.ts');
	const generated = await readProjectFile('src/lib/api/generated/school-api.ts');

	for (const name of [
		'CalendarEvent',
		'CalendarViewerEvent',
		'CalendarPublicEvent',
		'CalendarCategory',
		'CalendarTag',
		'CalendarEventTag',
		'CalendarEventTarget',
		'CalendarEventTargetInput',
		'CreateCalendarEventRequest',
		'listCalendarEvents',
		'listMyCalendarEvents',
		'listChildCalendarEvents',
		'listPublicCalendarEvents',
		'listCalendarTags',
		'createCalendarTag',
		'updateCalendarTag',
		'deleteCalendarTag'
	]) {
		assert.match(api, new RegExp(`\\b${name}\\b`));
	}

	const target = generatedSchemaBody(generated, 'CalendarEventTarget');
	const targetInput = interfaceBody(api, 'CalendarEventTargetInput');
	const viewerEvent = generatedSchemaBody(generated, 'CalendarViewerEvent');
	const publicFilters = interfaceBody(api, 'CalendarPublicEventFilters');
	const publicQuery = functionBody(api, 'publicCalendarQuery');
	const event = generatedSchemaBody(generated, 'CalendarEvent');
	const createEvent = interfaceBody(api, 'CreateCalendarEventRequest');

	assert.match(target, /\bid:\s*string;/);
	assert.match(target, /\baudienceType:\s*string;/);
	assert.match(target, /\bclassRoomId:\s*string \| null;/);
	assert.doesNotMatch(target, /\baudience:\s*CalendarAudienceType;/);
	assert.doesNotMatch(target, /\bclassroomId:\s*string \| null;/);
	assert.match(
		api,
		/export\s+type\s+CalendarEventTarget\s*=\s*Omit<CalendarEventTargetDto, 'audienceType'>/
	);
	assert.match(api, /audienceType:\s*CalendarAudienceType;/);
	assert.match(targetInput, /\baudienceType:\s*CalendarAudienceType;/);
	assert.match(targetInput, /\bclassRoomId\?:\s*string \| null;/);
	assert.doesNotMatch(targetInput, /\baudience:\s*CalendarAudienceType;/);
	assert.doesNotMatch(targetInput, /\bclassroomId\?:\s*string \| null;/);
	assert.doesNotMatch(targetInput, /\bid[?:]?:\s*string;/);
	assert.match(api, /targets:\s*CalendarEventTargetInput\[];/);
	assert.match(event, /tags:\s*components\['schemas'\]\['CalendarEventTag'\]\[];/);
	assert.match(createEvent, /tagIds:\s*string\[];/);
	assert.match(api, /export\s+type\s+CalendarEvent\s*=\s*Omit<CalendarEventDto, 'targets'>/);
	assert.match(api, /export\s+type\s+CalendarViewerEvent\s*=\s*Schemas\['CalendarViewerEvent'\]/);
	assert.match(api, /export\s+type\s+CalendarEventTag\s*=\s*Schemas\['CalendarEventTag'\]/);
	assert.match(viewerEvent, /tags:\s*components\['schemas'\]\['CalendarEventTag'\]\[];/);
	assert.doesNotMatch(viewerEvent, /targets:/);
	assert.doesNotMatch(viewerEvent, /reminders:/);
	assert.doesNotMatch(viewerEvent, /createdBy/);
	assert.doesNotMatch(viewerEvent, /updatedBy/);
	assert.match(api, /listMyCalendarEvents[\s\S]*Promise<CalendarViewerEvent\[]>/);
	assert.match(api, /listChildCalendarEvents[\s\S]*Promise<CalendarViewerEvent\[]>/);
	assert.match(api, /export\s+type\s+CalendarPublicEvent\s*=\s*Schemas\['CalendarPublicEvent'\]/);
	assert.doesNotMatch(api, /CalendarPublicEvent\s*=\s*Omit/);
	assert.match(publicFilters, /categoryId\?:\s*string;/);
	assert.match(publicFilters, /tagId\?:\s*string;/);
	assert.match(publicQuery, /params\.set\(['"]tag_id['"], filters\.tagId\)/);
	assert.doesNotMatch(publicFilters, /audience\?:/);
	assert.doesNotMatch(publicFilters, /visibility\?:/);
	assert.doesNotMatch(publicQuery, /\baudience\b/);
	assert.doesNotMatch(publicQuery, /\bvisibility\b/);
	assert.match(api, /listPublicCalendarEvents[\s\S]*Promise<CalendarPublicEvent\[]>/);
	assert.match(api, /listPublicCalendarEvents[\s\S]*publicCalendarQuery\(filters\)/);
});

test('calendar shared components use shadcn primitives', async () => {
	const monthGrid = await readProjectFile('src/lib/components/calendar/CalendarMonthGrid.svelte');
	const eventList = await readProjectFile('src/lib/components/calendar/CalendarEventList.svelte');
	const eventDialog = await readProjectFile(
		'src/lib/components/calendar/CalendarEventDialog.svelte'
	);
	const categoryDialog = await readProjectFile(
		'src/lib/components/calendar/CalendarCategoryDialog.svelte'
	);
	const colorKey = await readProjectFile('src/lib/components/calendar/CalendarColorKey.svelte');
	const staffPage = await readProjectFile('src/routes/(app)/staff/calendar/+page.svelte');

	assert.match(monthGrid, /buildCalendarMonthWeeks/);
	assert.match(monthGrid, /eventOverlapsDate/);
	assert.match(monthGrid, /CalendarDisplayEvent/);
	assert.match(monthGrid, /continuesFromPreviousWeek/);
	assert.match(monthGrid, /continuesIntoNextWeek/);
	assert.match(monthGrid, /style:grid-column/);
	assert.match(monthGrid, /hiddenEventCounts/);
	assert.match(monthGrid, /auto-rows-\[13px\]/);
	assert.match(monthGrid, /fillHeight/);
	assert.match(monthGrid, /sm:hidden[\s\S]*segment\.event\.title/);
	assert.match(monthGrid, /hidden truncate[\s\S]*sm:block[\s\S]*segmentLabel\(segment\)/);
	assert.doesNotMatch(monthGrid, /from '\$lib\/api\/calendar'/);
	assert.match(eventList, /from '\$lib\/components\/ui\/badge'/);
	assert.match(eventList, /from '\$lib\/components\/ui\/button'/);
	assert.match(eventList, /PageState/);
	assert.match(eventList, /Pencil/);
	assert.match(eventList, /Trash2/);
	assert.match(eventList, /showFullDescription/);
	assert.doesNotMatch(eventList, /from '\$lib\/api\/calendar'/);
	assert.match(eventDialog, /from '\$lib\/components\/ui\/dialog'/);
	assert.match(eventDialog, /from '\$lib\/components\/ui\/select'/);
	assert.match(eventDialog, /from '\$lib\/components\/ui\/checkbox'/);
	assert.match(eventDialog, /from '\$lib\/components\/ui\/button'/);
	assert.match(eventDialog, /CalendarEventTargetInput/);
	assert.match(categoryDialog, /from '\$lib\/components\/ui\/dialog'/);
	assert.match(categoryDialog, /from '\$lib\/components\/ui\/button'/);
	assert.match(categoryDialog, /UpsertCalendarCategoryRequest/);
	assert.match(categoryDialog, /UpsertCalendarTagRequest/);
	assert.match(categoryDialog, /from '\$lib\/components\/ui\/tabs'/);
	assert.match(categoryDialog, /from '\$lib\/components\/ui\/alert-dialog'/);
	assert.match(colorKey, /CalendarColorKeyItem/);
	assert.doesNotMatch(colorKey, /คำอธิบายสี/);
	assert.match(colorKey, /aria-label="หมวดหมู่กิจกรรมในปฏิทิน"/);
	assert.match(colorKey, /\{#each items as item \(item\.id\)\}/);
	assert.match(colorKey, /overflow-x-auto/);
	assert.match(colorKey, /sm:flex-wrap/);
	assert.match(staffPage, /CalendarColorKey/);
	assert.match(staffPage, /items=\{activeCategories\}/);
});

test('calendar event dialog builds backend-safe event payloads', async () => {
	const eventDialog = await readProjectFile(
		'src/lib/components/calendar/CalendarEventDialog.svelte'
	);

	assert.match(eventDialog, /function targetGradeLevelId\(audienceType: CalendarAudienceType\)/);
	assert.match(eventDialog, /function targetClassRoomId\(audienceType: CalendarAudienceType\)/);
	assert.match(eventDialog, /selectedClassRoomId \? null : selectedGradeLevelId \|\| null/);
	assert.match(eventDialog, /selectedClassRoomId \|\| null/);
	assert.match(eventDialog, /function changeGradeLevel\(value: string \| undefined\)/);
	assert.match(eventDialog, /selectedClassRoomId = ''/);
	assert.match(eventDialog, /notifyAudience = source \? false : true;/);
	assert.match(eventDialog, /selectedTagIds = source\?\.tags\.map\(\(tag\) => tag\.id\) \?\? \[]/);
	assert.match(eventDialog, /tagIds: selectedTagIds/);
	assert.match(eventDialog, /function toggleTag\(tagId: string\)/);
	assert.match(eventDialog, /loadEvent\(untrack\(\(\) => event\)\);/);
	assert.doesNotMatch(eventDialog, /onOpenChange=\{handleOpenChange\}/);
	assert.match(eventDialog, /hasMultipleTargetRows/);
	assert.match(eventDialog, /disabled=\{hasMultipleTargetRows/);
	assert.match(eventDialog, /ไม่สามารถแก้ไขกลุ่มผู้ชมหลายรายการ/);
});

test('calendar permission registry and routes are wired', async () => {
	const registry = await readProjectFile('src/lib/permissions/registry.generated.ts');
	const staffPage = await readProjectFile('src/routes/(app)/staff/calendar/+page.svelte');
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
	assert.match(staffPage, /PageShell/);
	assert.match(staffPage, /PERMISSIONS\.CALENDAR_MANAGE_SCHOOL/);
	assert.doesNotMatch(stripComments(staffPage), /calendar\.(manage|read)\.school/);
});

test('calendar routes keep staff reads and local state filter-aware', async () => {
	const staffPage = await readProjectFile('src/routes/(app)/staff/calendar/+page.svelte');
	const staffSource = stripComments(staffPage);

	assert.match(staffSource, /onMount\(\(\) => {[\s\S]*loadCalendar\(\)/);
	assert.doesNotMatch(staffSource, /hasAttemptedInitialLoad/);
	assert.match(staffSource, /calendarGridRange\(selectedMonth\)/);
	assert.match(staffSource, /from '\$lib\/components\/ui\/alert-dialog'/);
	assert.match(staffSource, /function requestDeleteEvent/);
	assert.match(staffSource, /function confirmDeleteEvent/);
	assert.match(staffSource, /activeFilterCount/);
	assert.match(staffSource, /listCalendarTags\(\)/);
	assert.match(staffSource, /tagId: tagId \|\| undefined/);
	assert.match(staffSource, /async function ensureManageOptions\(\): Promise<boolean>/);
	assert.match(staffSource, /manageOptionsPromise/);
	assert.match(staffSource, /let eventDialogSession = \$state\(0\);/);
	assert.match(staffSource, /eventDialogSession \+= 1;\s*eventDialogOpen = true;/);
	assert.match(staffSource, /\{#key eventDialogSession\}[\s\S]*<CalendarEventDialog/);
	assert.match(staffSource, /const optionsReady = await ensureManageOptions\(\);/);
	assert.match(staffSource, /if \(!optionsReady\) return;/);
	assert.doesNotMatch(staffSource, /function replaceEvent/);
	assert.match(staffSource, /function eventMatchesCurrentFilters\(event: CalendarEvent\)/);
	assert.match(staffSource, /function patchSavedEvent\(event: CalendarEvent\)/);
	assert.match(staffSource, /eventMatchesCurrentFilters\(event\)/);
	const matcherBody = svelteFunctionBody(staffSource, 'eventMatchesCurrentFilters');
	assert.match(matcherBody, /event\.title/);
	assert.match(matcherBody, /event\.description/);
	assert.match(matcherBody, /event\.location/);
	assert.match(matcherBody, /event\.tags/);
	assert.doesNotMatch(matcherBody, /event\.categoryName/);
	assert.match(
		staffSource,
		/event\.targets\.some\(\(target\) => target\.audienceType === audience\)/
	);
	assert.match(staffSource, /categoryName: savedCategory\.name/);
	assert.match(staffSource, /categoryColor: savedCategory\.color/);
	assert.match(staffSource, /categoryId = '';\s*await loadCalendar\(\);/);
});

test('staff calendar copies the current school public URL with feedback', async () => {
	const staffPage = await readProjectFile('src/routes/(app)/staff/calendar/+page.svelte');
	const copyBody = svelteFunctionBody(staffPage, 'copyPublicCalendarLink');

	assert.match(staffPage, /from '\$app\/state'/);
	assert.match(staffPage, /Copy/);
	assert.match(staffPage, /คัดลอกลิงก์สาธารณะ/);
	assert.match(copyBody, /page\.url\.origin/);
	assert.match(copyBody, /await navigator\.clipboard\.writeText/);
	assert.match(copyBody, /toast\.success\('คัดลอกลิงก์ปฏิทินสาธารณะแล้ว'\)/);
	assert.match(copyBody, /toast\.error\('คัดลอกลิงก์ไม่สำเร็จ'\)/);
});

test('calendar read-only pages sort selected-day events consistently', async () => {
	for (const route of [
		'src/routes/(app)/student/calendar/+page.svelte',
		'src/routes/(app)/parent/student/[id]/calendar/+page.svelte'
	]) {
		const page = await readProjectFile(route);
		assert.match(
			page,
			/events\s*\.filter\(\(event\) => eventOverlapsDate\(event, selectedDate\)\)\s*\.sort\(\(left, right\) => left\.startDate\.localeCompare\(right\.startDate\)\)/
		);
	}

	const publicPage = await readProjectFile('src/routes/(public)/calendar/+page.svelte');
	assert.match(publicPage, /Number\(right\.allDay\) - Number\(left\.allDay\)/);
	assert.match(
		publicPage,
		/\(left\.startTime \?\? ''\)\.localeCompare\(right\.startTime \?\? ''\)/
	);
});

test('public calendar fills mobile viewport and opens selected days in a timeline dialog', async () => {
	const publicPage = await readProjectFile('src/routes/(public)/calendar/+page.svelte');
	const timelineDialog = await readProjectFile(
		'src/lib/components/calendar/CalendarDayTimelineDialog.svelte'
	);
	const monthGridPosition = publicPage.indexOf('<CalendarMonthGrid');
	const colorKeyPosition = publicPage.indexOf('<CalendarColorKey items={colorKeyItems} />');
	const detailPanelPosition = publicPage.indexOf('<aside');

	assert.match(publicPage, /max-w-screen-2xl/);
	assert.match(publicPage, /h-dvh overflow-hidden/);
	assert.match(publicPage, /fillHeight/);
	assert.match(publicPage, /CalendarDayTimelineDialog/);
	assert.match(publicPage, /CalendarColorKey/);
	assert.match(publicPage, /buildCalendarColorKey/);
	assert.match(
		publicPage,
		/const colorKeyItems = \$derived\(buildCalendarColorKey\(selectedMonth, events\)\)/
	);
	assert.match(publicPage, /<CalendarColorKey items=\{colorKeyItems\} \/>/);
	assert.match(publicPage, /function selectDate\(date: string\)/);
	assert.match(publicPage, /window\.matchMedia\('\(max-width: 1023px\)'\)/);
	assert.match(publicPage, /hidden min-h-0[\s\S]*lg:flex/);
	assert.doesNotMatch(publicPage, /grid-rows-\[minmax\(0,2fr\)/);
	assert.match(publicPage, /lg:grid-cols-\[minmax\(0,1fr\)_22rem\]/);
	assert.match(publicPage, /xl:grid-cols-\[minmax\(0,1fr\)_24rem\]/);
	assert.ok(monthGridPosition >= 0, 'Expected the public month grid');
	assert.ok(colorKeyPosition > monthGridPosition, 'Expected the public color key below the month grid');
	assert.ok(
		detailPanelPosition > colorKeyPosition,
		'Expected the color key in the left calendar column'
	);
	assert.match(publicPage, /class="flex min-h-0 min-w-0 flex-col gap-3"/);
	assert.match(publicPage, /class="min-h-0 flex-1"[\s\S]*<CalendarMonthGrid/);
	assert.match(publicPage, /showFullDescription/);
	assert.match(publicPage, /function goToToday\(\)/);

	assert.match(timelineDialog, /from '\$lib\/components\/ui\/dialog'/);
	assert.match(timelineDialog, /bind:open/);
	assert.match(timelineDialog, /allDayEvents/);
	assert.match(timelineDialog, /timedEvents/);
	assert.match(timelineDialog, /overflow-y-auto/);
});
