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

test('protected app route previews derive titles from route metadata automatically', async () => {
	const routePreview = await readProjectFile('src/lib/server/route-preview-meta.ts');
	const hook = await readProjectFile('src/hooks.server.ts');
	const appHtml = await readProjectFile('src/app.html');
	const assessmentsRoute = await readProjectFile(
		'src/routes/(app)/staff/academic/assessments/+page.ts'
	);

	assert.match(
		routePreview,
		/import\.meta\.glob\(['"]\/src\/routes\/\(app\)\/\*\*\/\+page\.ts['"],\s*\{\s*eager:\s*true\s*\}/,
		'route preview registry should be built from app route modules, not a hand-written URL map'
	);
	assert.match(
		routePreview,
		/_meta\?\.preview\?\.title\s*\?\?\s*_meta\?\.menu\?\.title/,
		'preview title should come from the page _meta object'
	);
	assert.match(
		routePreview,
		/routePathToMatcher/,
		'route preview lookup should support SvelteKit route patterns'
	);
	assert.match(
		routePreview,
		/data-schoolorbit-route-preview/,
		'injected tags should be marked so duplicate server-rendered tags can be replaced'
	);

	assert.match(hook, /getRoutePreviewMeta\(event\.url\.pathname\)/);
	assert.match(hook, /transformPageChunk/);
	assert.match(hook, /injectRoutePreviewMeta/);

	assert.match(appHtml, /schoolorbit-route-preview-meta/);
	assert.match(assessmentsRoute, /title:\s*'โครงสร้างคะแนน'/);
});
