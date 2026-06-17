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
