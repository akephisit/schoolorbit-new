import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const appRoutes = path.join(projectRoot, 'src/routes/(app)');

async function pageFiles(directory) {
	const files = [];
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		const fullPath = path.join(directory, entry.name);
		if (entry.isDirectory()) files.push(...(await pageFiles(fullPath)));
		else if (entry.name === '+page.svelte') files.push(fullPath);
	}
	return files;
}

async function sourceFiles(directory) {
	const files = [];
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		const fullPath = path.join(directory, entry.name);
		if (entry.isDirectory()) files.push(...(await sourceFiles(fullPath)));
		else if (/\.(?:svelte|ts)$/.test(entry.name)) files.push(fullPath);
	}
	return files;
}

test('legacy onMount primary reads only shrink during route migration', async () => {
	const violations = [];
	for (const file of await pageFiles(appRoutes)) {
		const source = await readFile(file, 'utf8');
		if (/\bonMount\b/.test(source) && /\$lib\/api\//.test(source)) {
			violations.push(path.relative(projectRoot, file));
		}
	}
	violations.sort();
	assert.equal(violations.length, 71, violations.join('\n'));
	assert.equal(
		createHash('sha256').update(violations.join('\n')).digest('hex'),
		'3f6cfffc91f19190281984451c57d0cc6a4780cdd63b0283c65cdbb174c12734',
		violations.join('\n')
	);
	assert.ok(
		!violations.includes('src/routes/(app)/staff/academic/delivery/+page.svelte'),
		'Delivery must keep its primary read in +page.ts'
	);
});

test('the application keeps safe hover data preload enabled', async () => {
	const app = await readFile(path.join(projectRoot, 'src/app.html'), 'utf8');
	assert.match(app, /<body[^>]*data-sveltekit-preload-data="hover"/);
});

test('route and component code uses focused invalidation', async () => {
	const allowedInvalidateAllOwners = new Set([]);
	const offenders = [];
	for (const file of await sourceFiles(path.join(projectRoot, 'src'))) {
		const relative = path.relative(projectRoot, file);
		if (
			/\binvalidateAll\s*\(/.test(await readFile(file, 'utf8')) &&
			!allowedInvalidateAllOwners.has(relative)
		) {
			offenders.push(relative);
		}
	}
	assert.deepEqual(offenders.sort(), []);
});
