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

test('shared frontend state components use local shadcn-svelte primitives', async () => {
	const pageState = await readProjectFile('src/lib/components/app-state/PageState.svelte');
	const pageSkeleton = await readProjectFile('src/lib/components/app-state/PageSkeleton.svelte');
	const tableSkeleton = await readProjectFile('src/lib/components/app-state/TableSkeleton.svelte');
	const loadingButton = await readProjectFile('src/lib/components/app-state/LoadingButton.svelte');
	const appStateIndex = await readProjectFile('src/lib/components/app-state/index.ts');
	const skeletonIndex = await readProjectFile('src/lib/components/ui/skeleton/index.ts');
	const skeleton = await readProjectFile('src/lib/components/ui/skeleton/skeleton.svelte');

	assert.match(pageState, /from '\$lib\/components\/ui\/alert'/);
	assert.match(pageState, /from '\$lib\/components\/ui\/button'/);
	assert.match(pageState, /from '\$lib\/components\/ui\/card'/);

	assert.match(pageSkeleton, /from '\$lib\/components\/ui\/card'/);
	assert.match(pageSkeleton, /from '\$lib\/components\/ui\/skeleton'/);

	assert.match(tableSkeleton, /from '\$lib\/components\/ui\/table'/);
	assert.match(tableSkeleton, /from '\$lib\/components\/ui\/skeleton'/);

	assert.match(loadingButton, /from '\$lib\/components\/ui\/button'/);
	assert.match(loadingButton, /LoaderCircle/);

	assert.match(appStateIndex, /PageState/);
	assert.match(appStateIndex, /PageSkeleton/);
	assert.match(appStateIndex, /TableSkeleton/);
	assert.match(appStateIndex, /LoadingButton/);

	assert.match(skeletonIndex, /Root as Skeleton/);
	assert.match(skeleton, /data-slot="skeleton"/);
	assert.match(skeleton, /animate-pulse/);
});

test('staff and student list workspaces use shared frontend state components', async () => {
	const pages = [
		'src/routes/(app)/staff/manage/+page.svelte',
		'src/routes/(app)/staff/students/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-state'/,
			`${page} should import shared app-state components`
		);
		assert.match(source, /<PageSkeleton\b/, `${page} should use PageSkeleton for initial loading`);
		assert.match(source, /<PageState\b/, `${page} should use PageState for empty/error states`);
	}
});

test('academic top-level workspaces use shared frontend state components', async () => {
	const pages = [
		'src/routes/(app)/staff/academic/admission/+page.svelte',
		'src/routes/(app)/staff/academic/classrooms/+page.svelte',
		'src/routes/(app)/staff/academic/periods/+page.svelte',
		'src/routes/(app)/staff/academic/structure/+page.svelte',
		'src/routes/(app)/staff/academic/subject-groups/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-state'/,
			`${page} should import shared app-state components`
		);
		assert.match(source, /<PageSkeleton\b/, `${page} should use PageSkeleton for loading`);
		assert.match(source, /<PageState\b/, `${page} should use PageState for empty/blocked states`);
	}
});

test('academic curriculum and planning workspaces use shared frontend state components', async () => {
	const pages = [
		'src/routes/(app)/staff/academic/activities/+page.svelte',
		'src/routes/(app)/staff/academic/enrollments/+page.svelte',
		'src/routes/(app)/staff/academic/planning/+page.svelte',
		'src/routes/(app)/staff/academic/study-plans/+page.svelte',
		'src/routes/(app)/staff/academic/timetable/templates/+page.svelte',
		'src/routes/(app)/staff/academic/timetable/scheduling-config/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-state'/,
			`${page} should import shared app-state components`
		);
		assert.match(source, /<PageSkeleton\b/, `${page} should use PageSkeleton for loading`);
		assert.match(source, /<PageState\b/, `${page} should use PageState for empty/blocked states`);
	}
});

test('academic large workspaces use shared frontend state components', async () => {
	const pages = [
		'src/routes/(app)/staff/academic/subjects/+page.svelte',
		'src/routes/(app)/staff/academic/supervision/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-state'/,
			`${page} should import shared app-state components`
		);
		assert.match(source, /<PageSkeleton\b/, `${page} should use PageSkeleton for loading`);
		assert.match(source, /<PageState\b/, `${page} should use PageState for empty/blocked states`);
	}
});

test('academic timetable workspace uses shared frontend state components', async () => {
	const page = 'src/routes/(app)/staff/academic/timetable/+page.svelte';
	const source = await readProjectFile(page);

	assert.match(
		source,
		/from '\$lib\/components\/app-state'/,
		`${page} should import shared app-state components`
	);
	assert.match(source, /<PageSkeleton\b/, `${page} should use PageSkeleton for loading`);
	assert.match(source, /<PageState\b/, `${page} should use PageState for empty/blocked states`);
});

test('staff administration workspaces use shared frontend state components', async () => {
	const pages = [
		'src/routes/(app)/staff/roles/+page.svelte',
		'src/routes/(app)/staff/organization/+page.svelte',
		'src/routes/(app)/staff/menu/+page.svelte',
		'src/routes/(app)/staff/features/+page.svelte',
		'src/routes/(app)/staff/school-settings/+page.svelte',
		'src/routes/(app)/staff/facility/buildings/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-state'/,
			`${page} should import shared app-state components`
		);
		assert.match(source, /<PageSkeleton\b/, `${page} should use PageSkeleton for loading`);
		assert.match(source, /<PageState\b/, `${page} should use PageState for empty/blocked states`);
	}
});

test('staff self-service workspaces use shared frontend state components', async () => {
	const pages = [
		'src/routes/(app)/staff/achievements/+page.svelte',
		'src/routes/(app)/staff/work/+page.svelte',
		'src/routes/(app)/staff/work/manage/+page.svelte',
		'src/routes/(app)/staff/profile/+page.svelte',
		'src/routes/(app)/staff/timetable/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-state'/,
			`${page} should import shared app-state components`
		);
		assert.match(source, /<PageSkeleton\b/, `${page} should use PageSkeleton for loading`);
		assert.match(source, /<PageState\b/, `${page} should use PageState for empty/blocked states`);
	}
});

test('staff detail workspaces use shared frontend state components', async () => {
	const pages = [
		'src/routes/(app)/staff/manage/[id]/+page.svelte',
		'src/routes/(app)/staff/organization/[id]/+page.svelte',
		'src/routes/(app)/staff/roles/[id]/+page.svelte',
		'src/routes/(app)/staff/students/[id]/+page.svelte',
		'src/routes/(app)/staff/view/[id]/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-state'/,
			`${page} should import shared app-state components`
		);
		assert.match(source, /<PageSkeleton\b/, `${page} should use PageSkeleton for loading`);
		assert.match(source, /<PageState\b/, `${page} should use PageState for empty/blocked states`);
	}
});

test('parent and student self-service workspaces use shared frontend state components', async () => {
	const pages = [
		'src/routes/(app)/parent/+page.svelte',
		'src/routes/(app)/parent/student/[id]/+page.svelte',
		'src/routes/(app)/parent/student/[id]/timetable/+page.svelte',
		'src/routes/(app)/student/+page.svelte',
		'src/routes/(app)/student/activities/+page.svelte',
		'src/routes/(app)/student/profile/+page.svelte',
		'src/routes/(app)/student/timetable/+page.svelte'
	];

	for (const page of pages) {
		const source = await readProjectFile(page);

		assert.match(
			source,
			/from '\$lib\/components\/app-state'/,
			`${page} should import shared app-state components`
		);
		assert.match(source, /<PageSkeleton\b/, `${page} should use PageSkeleton for loading`);
		assert.match(source, /<PageState\b/, `${page} should use PageState for empty/error states`);
	}
});
