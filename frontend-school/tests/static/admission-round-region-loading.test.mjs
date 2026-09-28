import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const read = (route, file) =>
	readFile(
		new URL(`../../src/routes/(app)/staff/academic/admission/${route}/${file}`, import.meta.url),
		'utf8'
	);

test('admission round list starts the year-scoped primary read in its route', async () => {
	const [route, page] = await Promise.all([read('', '+page.ts'), read('', '+page.svelte')]);
	assert.match(route, /listRounds\(/);
	assert.match(route, /requestFetch: fetch/);
	assert.match(page, /data\.rounds/);
	assert.doesNotMatch(page, /academicContext\.subscribe\(/);
});

test('admission round detail starts independent visible regions in its route', async () => {
	const [route, page] = await Promise.all([read('[id]', '+page.ts'), read('[id]', '+page.svelte')]);
	for (const method of ['getRound', 'listTracks', 'listSubjects'])
		assert.match(route, new RegExp(`${method}\\(`));
	assert.match(route, /requestFetch: fetch/);
	assert.match(page, /data\.round/);
	assert.doesNotMatch(page, /onMount\(\(\) => \{\s*void load\(\)/);
});

test('admission create form starts authorized year options in its route', async () => {
	const [route, page] = await Promise.all([read('new', '+page.ts'), read('new', '+page.svelte')]);
	assert.match(route, /lookupAcademicYears\(/);
	assert.match(route, /requestFetch: fetch/);
	assert.match(page, /data\.years/);
	assert.doesNotMatch(page, /onMount\(load\)/);
});
