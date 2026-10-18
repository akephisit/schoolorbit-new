import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import ts from 'typescript';

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
		/import\.meta\.glob\(['"]\/src\/routes\/\[\(\]app\[\)\]\/\*\*\/\+page\.ts['"],\s*\{\s*eager:\s*true\s*\}/,
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

test('wide school card defaults preserve explicit image overrides and replace all managed tags', async () => {
	const source = (await readProjectFile('src/lib/server/route-preview-meta.ts')).replace(
		/import\.meta\.glob\([\s\S]*?\) as Record<string, PageRouteModule>/,
		`({
			'/src/routes/(app)/staff/example/+page.ts': { _meta: { menu: { title: 'เมนูทดสอบ' } } },
			'/src/routes/(app)/staff/custom/+page.ts': { _meta: { preview: { title: 'กำหนดเอง', image: 'https://images.example/square.jpg' } } }
		})`
	);
	const { outputText } = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 }
	});
	const { getRoutePreviewMeta, injectRoutePreviewMeta } = await import(
		`data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`
	);
	const url = new URL('https://alpha.schoolorbit.app/staff/example');
	const meta = getRoutePreviewMeta(url.pathname);
	assert.equal(meta.title, 'เมนูทดสอบ');
	assert.equal(meta.image, '/og-image.png');
	const html = injectRoutePreviewMeta('<head><title>old</title></head>', meta, url);
	assert.match(html, /<title>เมนูทดสอบ - SchoolOrbit<\/title>/);
	assert.match(html, /og:image" content="https:\/\/alpha\.schoolorbit\.app\/og-image\.png/);
	assert.match(html, /og:image:width" content="1200"/);
	assert.match(html, /og:image:height" content="630"/);
	assert.match(html, /og:image:type" content="image\/png"/);
	assert.match(html, /twitter:card" content="summary_large_image"/);
	const custom = getRoutePreviewMeta('/staff/custom');
	const overridden = injectRoutePreviewMeta(html, custom, url);
	assert.match(overridden, /og:image" content="https:\/\/images\.example\/square\.jpg"/);
	assert.match(overridden, /twitter:card" content="summary"/);
	assert.doesNotMatch(overridden, /og:image:(width|height|type|alt)|twitter:image:alt/);
	assert.equal((overridden.match(/<title>/g) || []).length, 1);
	const escaped = injectRoutePreviewMeta(
		'<head></head>',
		{ ...meta, title: '<script>"&', description: '"><script>' },
		url
	);
	assert.doesNotMatch(escaped, /<script>/);
	assert.match(escaped, /&lt;script&gt;/);
});
