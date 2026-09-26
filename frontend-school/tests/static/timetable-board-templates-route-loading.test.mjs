import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const root = path.resolve(import.meta.dirname, '../..');

function source(relativePath) {
	return readFile(path.join(root, relativePath), 'utf8');
}

test('board route starts versions and explicit selected workspace before component mount', async () => {
	const directory = 'src/routes/(app)/staff/academic/timetable';
	const [loader, page] = await Promise.all([
		source(`${directory}/+page.ts`),
		source(`${directory}/+page.svelte`)
	]);
	assert.match(loader, /listTimetableVersions/);
	assert.match(loader, /getTimetableBlockWorkspace/);
	assert.match(loader, /getAcademicTermChangeSet/);
	assert.match(loader, /requestFetch:\s*fetch/);
	assert.match(page, /data\.versions/);
	assert.match(page, /data\.workspace/);
	assert.match(page, /data\.changeSet/);
	const mountOnly = page.slice(page.lastIndexOf('\tonMount('), page.indexOf('</script>'));
	assert.doesNotMatch(
		mountOnly,
		/\b(?:listTimetableVersions|getTimetableBlockWorkspace|getAcademicTermChangeSet|loadContext|loadVersion)\s*\(/
	);
});

test('template list and versions are independently route-owned and export stays lazy', async () => {
	const directory = 'src/routes/(app)/staff/academic/timetable/templates';
	const [loader, page, api, board] = await Promise.all([
		source(`${directory}/+page.ts`),
		source(`${directory}/+page.svelte`),
		source('src/lib/api/timetable.ts'),
		source('src/routes/(app)/staff/academic/timetable/+page.svelte')
	]);
	assert.match(loader, /listTimetableTemplates/);
	assert.match(loader, /listTimetableVersions/);
	assert.match(loader, /permission: PERMISSIONS\.ACADEMIC_TIMETABLE_READ_SCHOOL/);
	assert.match(loader, /requestFetch:\s*fetch/);
	assert.match(page, /data\.templates/);
	assert.match(page, /data\.versions/);
	assert.match(page, /PERMISSIONS\.ACADEMIC_TIMETABLE_MANAGE_SCHOOL/);
	const mountOnly = page.slice(page.lastIndexOf('\tonMount('), page.indexOf('</script>'));
	assert.doesNotMatch(
		mountOnly,
		/\b(?:listTimetableTemplates|listTimetableVersions|loadTemplates|loadVersions)\s*\(/
	);
	assert.match(api, /listTimetableTemplates[\s\S]{0,100}ApiRequestOptions/);
	assert.match(board, /await import\('\$lib\/utils\/timetable-teacher-load-workbook'\)/);
});

test('direct timetable links carry the already-known academic context', async () => {
	const [
		todayLoader,
		todayPage,
		templatesLoader,
		templatesPage,
		changeReadiness,
		supervision,
		pageShell,
		pageHeader
	] = await Promise.all([
		source('src/routes/(app)/staff/academic/timetable/today/+page.ts'),
		source('src/routes/(app)/staff/academic/timetable/today/+page.svelte'),
		source('src/routes/(app)/staff/academic/timetable/templates/+page.ts'),
		source('src/routes/(app)/staff/academic/timetable/templates/+page.svelte'),
		source('src/lib/components/learning-delivery/AcademicChangeReadiness.svelte'),
		source('src/lib/components/supervision/SupervisionWorkspace.svelte'),
		source('src/lib/components/app-layout/PageShell.svelte'),
		source('src/lib/components/app-layout/PageHeader.svelte')
	]);
	assert.match(todayLoader, /academicYearId/);
	assert.match(todayPage, /academicYearId/);
	assert.match(templatesLoader, /academicYearId/);
	assert.match(templatesPage, /backHref=\{[^}]*academicYearId/);
	assert.match(templatesPage, /backPreload="tap"/);
	assert.match(pageShell, /<PageHeader[^>]*\{backPreload\}/);
	assert.match(pageHeader, /data-sveltekit-preload-data=\{backPreload\}/);
	assert.match(changeReadiness, /changeSet\.academicYearId/);
	assert.match(changeReadiness, /changeSet\.academicTermId/);
	assert.match(supervision, /academicContextualMenuPath\(/);
});
