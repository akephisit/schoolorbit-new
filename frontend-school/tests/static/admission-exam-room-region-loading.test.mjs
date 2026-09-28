import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const readRoute = (file) =>
	readFile(
		new URL(
			`../../src/routes/(app)/staff/academic/admission/[id]/exam-rooms/${file}`,
			import.meta.url
		),
		'utf8'
	);

test('exam room route starts independent setup regions and leaves optional data lazy', async () => {
	const [route, page] = await Promise.all([readRoute('+page.ts'), readRoute('+page.svelte')]);
	for (const method of ['getRound', 'listExamRooms', 'getExamConfig'])
		assert.match(route, new RegExp(`${method}\\(`));
	assert.match(route, /requestFetch: fetch/);
	assert.doesNotMatch(route, /getExamSeats\(/);
	assert.doesNotMatch(route, /listRooms\(/);
	assert.doesNotMatch(route, /listRounds\(/);
	for (const region of ['round', 'rooms', 'config'])
		assert.match(page, new RegExp(`data\\.${region}`));
	assert.doesNotMatch(page, /onMount\(\(\) => \{\s*loadAll\(\)/);
});
