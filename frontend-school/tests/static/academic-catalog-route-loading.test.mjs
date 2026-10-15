import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const routesRoot = path.resolve(
	import.meta.dirname,
	'../../src/routes/(app)/staff/academic/catalog'
);
const cases = [
	{
		route: 'subjects',
		read: 'getCatalogSubjectOverview',
		result: 'overview',
		dependency: 'CATALOG_SUBJECT_OVERVIEW_DEPENDENCY',
		lazy: 'listSubjectVersions'
	},
	{
		route: 'activities',
		read: 'getCatalogActivityOverview',
		result: 'overview',
		dependency: 'CATALOG_ACTIVITY_OVERVIEW_DEPENDENCY',
		lazy: 'listActivityVersions'
	}
];

test('catalog list regions start in route loaders and keep history lazy', async () => {
	for (const entry of cases) {
		const directory = path.join(routesRoot, entry.route);
		const loader = await readFile(path.join(directory, '+page.ts'), 'utf8');
		const page = await readFile(path.join(directory, '+page.svelte'), 'utf8');
		assert.match(loader, new RegExp(`\\b${entry.read}\\b`), entry.route);
		assert.match(loader, /captureRouteLoad\(/, entry.route);
		assert.match(loader, /requestFetch:\s*fetch/, entry.route);
		assert.match(loader, new RegExp(`depends\\(${entry.dependency}\\)`), entry.route);
		assert.match(loader, new RegExp(`${entry.result}\\s*:`), entry.route);
		assert.doesNotMatch(page, new RegExp(`\\b${entry.read}\\s*\\(`), entry.route);
		assert.doesNotMatch(page, /\bonMount\s*\(/, entry.route);
		if (entry.lazy) {
			assert.match(page, new RegExp(`\\b${entry.lazy}\\s*\\(`), entry.route);
		}
	}
});

test('subject-group menu redirects to the single organization registry', async () => {
	const loader = await readFile(path.join(routesRoot, 'subject-groups/+page.ts'), 'utf8');
	assert.match(loader, /redirect\(307/);
	assert.match(loader, /staff\/organization/);
	assert.doesNotMatch(loader, /listSubjectGroups/);
});

test('retained catalog data exposes a focused retry after a refresh error', async () => {
	for (const entry of cases) {
		const page = await readFile(path.join(routesRoot, entry.route, '+page.svelte'), 'utf8');
		assert.match(
			page,
			new RegExp(
				`errorMessage\\}[^]*?role="alert"[^]*?onclick=\\{${entry.route === 'subject-groups' ? 'loadGroups' : 'loadOverview'}\\}[^]*?ลองอีกครั้ง`
			),
			entry.route
		);
	}
});
