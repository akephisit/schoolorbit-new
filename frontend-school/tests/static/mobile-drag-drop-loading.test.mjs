import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '../..');

async function readProjectFile(relativePath) {
	try {
		return await readFile(path.join(projectRoot, relativePath), 'utf8');
	} catch (error) {
		if (error?.code === 'ENOENT') return '';
		throw error;
	}
}

const dragDropRoutes = [
	'src/routes/(app)/staff/academic/admission/[id]/selections/+page.svelte',
	'src/routes/(app)/staff/academic/periods/+page.svelte',
	'src/routes/(app)/staff/academic/timetable/+page.svelte',
	'src/routes/(app)/staff/academic/timetable/scheduling-config/+page.svelte',
	'src/routes/(app)/staff/menu/+page.svelte',
	'src/routes/(app)/staff/organization/+page.svelte'
];

test('mobile drag drop polyfill is not loaded by the authenticated app layout', async () => {
	const source = await readProjectFile('src/routes/(app)/+layout.svelte');

	assert.doesNotMatch(source, /mobile-drag-drop/);
	assert.doesNotMatch(source, /cdn\.jsdelivr\.net/);
	assert.doesNotMatch(source, /enableMobileDragDrop/);
});

test('mobile drag drop opt-in component uses package-local assets', async () => {
	const source = await readProjectFile('src/lib/components/MobileDragDropPolyfill.svelte');

	assert.match(source, /mobile-drag-drop\/default\.css/);
	assert.match(source, /await import\('mobile-drag-drop'\)/);
	assert.match(source, /mobile-drag-drop\/scroll-behaviour/);
	assert.doesNotMatch(source, /https?:\/\//);
});

test('native drag drop app routes opt into the mobile polyfill explicitly', async () => {
	for (const route of dragDropRoutes) {
		const source = await readProjectFile(route);

		assert.match(
			source,
			/MobileDragDropPolyfill/,
			`${route} should render MobileDragDropPolyfill only where native drag/drop is used`
		);
	}
});
