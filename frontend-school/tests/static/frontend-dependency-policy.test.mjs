import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

async function sourceFiles(relativeDirectory) {
	const root = path.join(projectRoot, relativeDirectory);
	const files = [];
	for (const entry of await readdir(root, { withFileTypes: true, recursive: true })) {
		if (!entry.isFile() || !/\.(?:svelte|[cm]?[jt]s)$/.test(entry.name)) continue;
		files.push(path.join(entry.parentPath, entry.name));
	}
	return files;
}

test('frontend sources use the canonical Lucide package owner', async () => {
	const packageJson = JSON.parse(await readFile(path.join(projectRoot, 'package.json'), 'utf8'));
	assert.equal(packageJson.dependencies?.['lucide-svelte'], undefined);
	assert.match(packageJson.devDependencies?.['@lucide/svelte'] ?? '', /^\^1\.47\.0$/);

	for (const file of await Promise.all([
		sourceFiles('src'),
		sourceFiles('tests'),
		sourceFiles('scripts')
	]).then((sets) => sets.flat())) {
		assert.doesNotMatch(
			await readFile(file, 'utf8'),
			/(?:from\s+|import\s*\(\s*)['"]lucide-svelte(?:\/[^'"]*)?['"]/
		);
	}
});

test('dynamic icon consumers use the public Lucide component type', async () => {
	for (const relativePath of [
		'src/lib/utils/icon-mapper.ts',
		'src/lib/components/consent/StatusBadge.svelte'
	]) {
		const source = await readFile(path.join(projectRoot, relativePath), 'utf8');
		assert.match(source, /import type \{ LucideIcon \} from '@lucide\/svelte'/);
		assert.doesNotMatch(source, /Icon as LucideIcon/);
	}
});

test('SheetJS resolves from the immutable approved artifact', async () => {
	const packageJson = JSON.parse(await readFile(path.join(projectRoot, 'package.json'), 'utf8'));
	const packageLock = JSON.parse(
		await readFile(path.join(projectRoot, 'package-lock.json'), 'utf8')
	);
	const artifactUrl = 'https://cdn.sheetjs.com/xlsx-0.20.3/xlsx-0.20.3.tgz';
	assert.equal(packageJson.dependencies?.xlsx, artifactUrl);
	assert.equal(packageLock.packages?.['node_modules/xlsx']?.version, '0.20.3');
	assert.equal(packageLock.packages?.['node_modules/xlsx']?.resolved, artifactUrl);
	assert.match(packageLock.packages?.['node_modules/xlsx']?.integrity ?? '', /^sha512-/);
});
