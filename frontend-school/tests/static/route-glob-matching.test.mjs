import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { glob } from 'tinyglobby';

const root = path.resolve(import.meta.dirname, '../..');
async function routeFiles(directory) {
	const found = [];
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		const file = path.join(directory, entry.name);
		if (entry.isDirectory()) found.push(...(await routeFiles(file)));
		else if (entry.name === '+page.ts') found.push(path.relative(root, file));
	}
	return found.sort();
}
test('Vite metadata registries match every app route despite literal group parentheses', async () => {
	const expected = await routeFiles(path.join(root, 'src/routes/(app)'));
	assert.ok(expected.length > 0);
	for (const owner of [
		'src/lib/auth/route-access.ts',
		'src/lib/server/route-preview-meta.ts',
		'src/lib/academic-context/route-context.ts'
	]) {
		const source = await readFile(path.join(root, owner), 'utf8');
		const literal = source.match(/import\.meta\.glob\(['"]([^'"]+)['"]/);
		assert.ok(literal, `${owner}: literal metadata pattern required`);
		const actual = await glob(literal[1].replace(/^\//, ''), { cwd: root });
		assert.deepEqual(
			actual.sort(),
			expected,
			`${owner}: metadata must cover exactly the app routes`
		);
	}
});
