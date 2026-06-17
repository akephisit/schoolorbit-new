import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '../..');
const repoRoot = path.resolve(projectRoot, '..');

async function readProjectFile(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

async function readRepoFile(relativePath) {
	return readFile(path.join(repoRoot, relativePath), 'utf8');
}

test('shared app layout components define consistent page header and shell', async () => {
	const pageHeader = await readProjectFile('src/lib/components/app-layout/PageHeader.svelte');
	const pageShell = await readProjectFile('src/lib/components/app-layout/PageShell.svelte');
	const appLayoutIndex = await readProjectFile('src/lib/components/app-layout/index.ts');
	const rules = await readRepoFile('.rules');

	assert.match(pageHeader, /from '\$lib\/components\/ui\/button'/);
	assert.match(pageHeader, /from '\$lib\/utils'/);
	assert.match(pageHeader, /ArrowLeft/);
	assert.match(pageHeader, /text-2xl/);
	assert.match(pageHeader, /tracking-tight/);
	assert.match(pageHeader, /actions\?: Snippet/);
	assert.match(pageHeader, /icon\?: Component/);

	assert.match(pageShell, /from '.\/PageHeader.svelte'/);
	assert.match(pageShell, /space-y-6/);
	assert.match(pageShell, /@render children/);

	assert.match(appLayoutIndex, /PageHeader/);
	assert.match(appLayoutIndex, /PageShell/);

	assert.match(rules, /Shared Page Layout UI/);
	assert.match(rules, /PageHeader/);
	assert.match(rules, /PageShell/);
});

test('pilot workspaces use shared app page shell', async () => {
	const pages = [
		'src/routes/(app)/staff/manage/+page.svelte',
		'src/routes/(app)/staff/students/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-layout'/,
			`${page} should import shared app-layout components`
		);
		assert.match(source, /<PageShell\b/, `${page} should use PageShell for page layout`);
		assert.doesNotMatch(
			source,
			/<div class="space-y-6">\s*<!-- Header -->/,
			`${page} should not hand-roll the page header wrapper`
		);
	}
});

test('staff core pages use shared app page shell', async () => {
	const pages = [
		'src/routes/(app)/staff/+page.svelte',
		'src/routes/(app)/staff/settings/+page.svelte',
		'src/routes/(app)/staff/profile/+page.svelte',
		'src/routes/(app)/staff/school-settings/+page.svelte',
		'src/routes/(app)/staff/features/+page.svelte',
		'src/routes/(app)/staff/menu/+page.svelte',
		'src/routes/(app)/staff/roles/+page.svelte',
		'src/routes/(app)/staff/manage/[id]/roles/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-layout'/,
			`${page} should import shared app-layout components`
		);
		assert.match(source, /<PageShell\b/, `${page} should use PageShell for page layout`);
		assert.doesNotMatch(
			source,
			/<div class="space-y-6">\s*<!-- Header -->/,
			`${page} should not hand-roll the page header wrapper`
		);
	}
});

test('staff people detail and action pages use shared app page shell', async () => {
	const pages = [
		'src/routes/(app)/staff/manage/[id]/+page.svelte',
		'src/routes/(app)/staff/manage/[id]/edit/+page.svelte',
		'src/routes/(app)/staff/manage/new/+page.svelte',
		'src/routes/(app)/staff/students/[id]/+page.svelte',
		'src/routes/(app)/staff/students/[id]/edit/+page.svelte',
		'src/routes/(app)/staff/students/new/+page.svelte',
		'src/routes/(app)/staff/roles/[id]/+page.svelte',
		'src/routes/(app)/staff/view/[id]/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-layout'/,
			`${page} should import shared app-layout components`
		);
		assert.match(source, /<PageShell\b/, `${page} should use PageShell for page layout`);
		assert.doesNotMatch(
			source,
			/<div class="space-y-6">\s*<!-- Header -->/,
			`${page} should not hand-roll the page header wrapper`
		);
	}
});

test('staff operational pages use shared app page shell', async () => {
	const pages = [
		'src/routes/(app)/staff/organization/+page.svelte',
		'src/routes/(app)/staff/organization/[id]/+page.svelte',
		'src/routes/(app)/staff/work/+page.svelte',
		'src/routes/(app)/staff/work/manage/+page.svelte',
		'src/routes/(app)/staff/achievements/+page.svelte',
		'src/routes/(app)/staff/facility/buildings/+page.svelte',
		'src/routes/(app)/staff/timetable/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-layout'/,
			`${page} should import shared app-layout components`
		);
		assert.match(source, /<PageShell\b/, `${page} should use PageShell for page layout`);
		assert.doesNotMatch(
			source,
			/<div class="space-y-6">\s*<!-- Header -->/,
			`${page} should not hand-roll the page header wrapper`
		);
	}
});

test('academic foundation workspace pages use shared app page shell', async () => {
	const pages = [
		'src/routes/(app)/staff/academic/structure/+page.svelte',
		'src/routes/(app)/staff/academic/classrooms/+page.svelte',
		'src/routes/(app)/staff/academic/subject-groups/+page.svelte',
		'src/routes/(app)/staff/academic/periods/+page.svelte',
		'src/routes/(app)/staff/academic/admission/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-layout'/,
			`${page} should import shared app-layout components`
		);
		assert.match(source, /<PageShell\b/, `${page} should use PageShell for page layout`);
		assert.doesNotMatch(
			source,
			/<div class="space-y-6">\s*<!-- Header -->/,
			`${page} should not hand-roll the page header wrapper`
		);
	}
});

test('academic curriculum planning pages use shared app page shell', async () => {
	const pages = [
		'src/routes/(app)/staff/academic/subjects/+page.svelte',
		'src/routes/(app)/staff/academic/subject-groups/[id]/+page.svelte',
		'src/routes/(app)/staff/academic/study-plans/+page.svelte',
		'src/routes/(app)/staff/academic/planning/+page.svelte',
		'src/routes/(app)/staff/academic/enrollments/+page.svelte',
		'src/routes/(app)/staff/academic/activities/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-layout'/,
			`${page} should import shared app-layout components`
		);
		assert.match(source, /<PageShell\b/, `${page} should use PageShell for page layout`);
		assert.doesNotMatch(
			source,
			/<div class="space-y-6">\s*<!-- Header -->/,
			`${page} should not hand-roll the page header wrapper`
		);
	}
});

test('academic timetable scheduling pages use shared app page shell', async () => {
	const pages = [
		'src/routes/(app)/staff/academic/timetable/+page.svelte',
		'src/routes/(app)/staff/academic/timetable/templates/+page.svelte',
		'src/routes/(app)/staff/academic/timetable/scheduling-config/+page.svelte',
		'src/routes/(app)/staff/academic/timetable/scheduling/jobs/+page.svelte',
		'src/routes/(app)/staff/academic/timetable/scheduling/jobs/[jobId]/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-layout'/,
			`${page} should import shared app-layout components`
		);
		assert.match(source, /<PageShell\b/, `${page} should use PageShell for page layout`);
		assert.doesNotMatch(
			source,
			/<div class="space-y-6">\s*<!-- Header -->/,
			`${page} should not hand-roll the page header wrapper`
		);
	}
});

test('academic admission workflow pages use shared app page shell', async () => {
	const pages = [
		'src/routes/(app)/staff/academic/admission/new/+page.svelte',
		'src/routes/(app)/staff/academic/admission/[id]/+page.svelte',
		'src/routes/(app)/staff/academic/admission/[id]/applications/+page.svelte',
		'src/routes/(app)/staff/academic/admission/[id]/applications/[appId]/+page.svelte',
		'src/routes/(app)/staff/academic/admission/[id]/enrollment/+page.svelte',
		'src/routes/(app)/staff/academic/admission/[id]/exam-rooms/+page.svelte',
		'src/routes/(app)/staff/academic/admission/[id]/report/+page.svelte',
		'src/routes/(app)/staff/academic/admission/[id]/scores/+page.svelte',
		'src/routes/(app)/staff/academic/admission/[id]/selections/+page.svelte',
		'src/routes/(app)/staff/academic/admission/[id]/student-ids/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-layout'/,
			`${page} should import shared app-layout components`
		);
		assert.match(source, /<PageShell\b/, `${page} should use PageShell for page layout`);
		assert.doesNotMatch(
			source,
			/<div class="space-y-6">\s*<!-- Header -->/,
			`${page} should not hand-roll the page header wrapper`
		);
	}
});

test('academic detail and supervision pages use shared app page shell', async () => {
	const pages = [
		'src/routes/(app)/staff/academic/activities/[id]/+page.svelte',
		'src/routes/(app)/staff/academic/supervision/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-layout'/,
			`${page} should import shared app-layout components`
		);
		assert.match(source, /<PageShell\b/, `${page} should use PageShell for page layout`);
		assert.doesNotMatch(
			source,
			/<div class="space-y-6">\s*<!-- Header -->/,
			`${page} should not hand-roll the page header wrapper`
		);
	}
});

test('self-service and system pages use shared app page shell', async () => {
	const pages = [
		'src/routes/(app)/parent/+page.svelte',
		'src/routes/(app)/parent/student/[id]/+page.svelte',
		'src/routes/(app)/parent/student/[id]/timetable/+page.svelte',
		'src/routes/(app)/student/+page.svelte',
		'src/routes/(app)/student/activities/+page.svelte',
		'src/routes/(app)/student/profile/+page.svelte',
		'src/routes/(app)/student/settings/+page.svelte',
		'src/routes/(app)/student/timetable/+page.svelte',
		'src/routes/(app)/settings/consent/+page.svelte',
		'src/routes/(app)/403/+page.svelte',
		'src/routes/(app)/debug/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-layout'/,
			`${page} should import shared app-layout components`
		);
		assert.match(source, /<PageShell\b/, `${page} should use PageShell for page layout`);
		assert.doesNotMatch(
			source,
			/<div class="space-y-6">\s*<!-- Header -->/,
			`${page} should not hand-roll the page header wrapper`
		);
	}
});
